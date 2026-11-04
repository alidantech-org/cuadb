//! Validated server-side row paging endpoint. Never accepts raw SQL.
use askama::Template;
use axum::{
    extract::{RawQuery, State},
    http::{header, HeaderValue},
    response::{Html, IntoResponse, Response},
};
use crate::{
    db::types::{DbError, DbErrorKind, RelationId, RowOptions, SortDirection, SortSpec},
    error::{AppError, ErrorKind},
    state::AppState,
};
#[derive(Clone)]
struct Header { name:String, native_type:String, asc:String, desc:String }
#[derive(Clone)]
struct Value { text:String, native_type:String, is_null:bool, truncated:bool }
#[derive(Clone)]
struct DisplayRow { number:u64, values:Vec<Value> }
#[derive(Template)]
#[template(path="components/data_page.html")]
struct PageTemplate {
    title:String,
    headers:Vec<Header>,
    rows:Vec<DisplayRow>,
    has_previous:bool,
    has_more:bool,
    previous_url:String,
    next_url:String,
    page_number:u32,
    size_25:String,
    size_50:String,
    size_100:String,
    stable:bool,
    keyset:bool,
    truncated_columns:bool,
    limit:u32,
    shown_from:u64,
    shown_to:u64,
}
fn invalid()->AppError { AppError::new(ErrorKind::InvalidInput) }
fn classified(db:DbError)->AppError {
    AppError::new(match db.kind {
        DbErrorKind::InvalidIdentifier=>ErrorKind::InvalidInput,
        DbErrorKind::PermissionDenied=>ErrorKind::Forbidden,
        DbErrorKind::TimedOut=>ErrorKind::TimedOut,
        DbErrorKind::ConnectionUnavailable|DbErrorKind::AuthenticationFailed=>
            ErrorKind::DatabaseUnavailable,
        DbErrorKind::Unsupported=>ErrorKind::InvalidInput,
        DbErrorKind::Internal=>ErrorKind::Internal,
    })
}
fn name(value:&str)->Result<String,AppError> {
    if value.is_empty() || value.len()>63 || value.chars().any(char::is_control) {
        Err(invalid())
    } else { Ok(value.to_string()) }
}
#[derive(Clone)]
struct Navigation {
    id:RelationId,
    options:RowOptions,
}
fn parse(raw:Option<String>)->Result<Navigation,AppError> {
    let raw=raw.ok_or_else(invalid)?;
    if raw.len()>2048 {return Err(invalid());}
    let (mut schema,mut relation,mut sort,mut direction)=(None,None,None,None);
    let (mut limit,mut page,mut cursor)=(None,None,None);
    let mut columns=Vec::new();
    for (key,value) in url::form_urlencoded::parse(raw.as_bytes()) {
        let v=value.as_ref();
        match key.as_ref() {
            "schema" if schema.is_none()=>schema=Some(name(v)?),
            "relation" if relation.is_none()=>relation=Some(name(v)?),
            "sort" if sort.is_none()=>sort=Some(name(v)?),
            "direction" if direction.is_none()=>{
                direction=Some(match v {
                    "asc"=>SortDirection::Asc,
                    "desc"=>SortDirection::Desc,
                    _=>return Err(invalid()),
                });
            },
            "size" if limit.is_none()=>{
                limit=Some(v.parse::<u32>().map_err(|_|invalid())?);
            },
            "page" if page.is_none()=>{
                page=Some(v.parse::<u32>().map_err(|_|invalid())?);
            },
            "cursor" if cursor.is_none()=>{
                if v.len()!=32 || !v.bytes().all(|b|b.is_ascii_hexdigit()) {
                    return Err(invalid());
                }
                cursor=Some(v.to_string());
            },
            "column" if columns.len()<24=>columns.push(name(v)?),
            _=>return Err(invalid()),
        }
    }
    if sort.is_some()!=direction.is_some() {return Err(invalid());}
    let options=RowOptions {
        limit:limit.unwrap_or(25),
        page:page.unwrap_or(0),
        selected_columns:columns,
        sort:sort.zip(direction).map(|(column,direction)|SortSpec{column,direction}),
        cursor,
    };
    if options.page>100 && options.cursor.is_none() {return Err(invalid());}
    Ok(Navigation {
        id:RelationId {
            namespace:schema.ok_or_else(invalid)?,
            name:relation.ok_or_else(invalid)?,
        },
        options,
    })
}
fn link(nav:&Navigation,page:u32,cursor:Option<&str>,sort:Option<SortSpec>)->String {
    let mut q=url::form_urlencoded::Serializer::new(String::new());
    q.append_pair("schema",&nav.id.namespace);
    q.append_pair("relation",&nav.id.name);
    q.append_pair("size",&nav.options.limit.to_string());
    if let Some(sort)=sort {
        q.append_pair("sort",&sort.column);
        q.append_pair("direction",match sort.direction {
            SortDirection::Asc=>"asc",SortDirection::Desc=>"desc",
        });
    }
    for col in &nav.options.selected_columns {q.append_pair("column",col);}
    if let Some(token)=cursor {q.append_pair("cursor",token);}
    else if page>0 {q.append_pair("page",&page.to_string());}
    format!("/catalog/data?{}",q.finish())
}
pub async fn data(State(state):State<AppState>,RawQuery(raw):RawQuery)
    ->Result<Response,AppError>
{
    let nav=parse(raw)?;
    let catalog=state.catalog().ok_or(AppError::new(ErrorKind::DatabaseUnavailable))?;
    let page=catalog.fetch_rows(&nav.id,nav.options.clone()).await.map_err(classified)?;
    let headers=page.columns.iter().map(|c|Header {
        name:c.name.clone(),native_type:c.native_type.clone(),
        asc:link(&nav,0,None,Some(SortSpec {
            column:c.name.clone(),direction:SortDirection::Asc,
        })),
        desc:link(&nav,0,None,Some(SortSpec {
            column:c.name.clone(),direction:SortDirection::Desc,
        })),
    }).collect::<Vec<_>>();
    let shown_from=if page.rows.is_empty() {0} else {
        u64::from(page.page)*u64::from(nav.options.limit)+1
    };
    let shown_to=if page.rows.is_empty() {0} else {
        shown_from+page.rows.len() as u64-1
    };
    let previous_url=if page.has_previous {
        link(&nav,page.page.saturating_sub(1),page.previous_cursor.as_deref(),
            nav.options.sort.clone())
    } else {String::new()};
    let next_url=if page.has_more && (page.page<100 || page.next_cursor.is_some()) {
        link(&nav,page.page.saturating_add(1),page.next_cursor.as_deref(),
            nav.options.sort.clone())
    } else {String::new()};
    let mut rendered=Vec::new();
    for (index,(cells,previews)) in page.rows.into_iter()
        .zip(page.previews).enumerate()
    {
        let values=cells.iter().zip(previews).map(|(cell,preview)|Value {
            text:preview.text,native_type:preview.type_name,
            truncated:preview.truncated,
            is_null:matches!(cell,crate::db::types::Cell::Null),
        }).collect();
        rendered.push(DisplayRow {number:shown_from+index as u64,values});
    }
    let mut sizes=nav.clone();
    sizes.options.limit=25;
    let size_25=link(&sizes,0,None,sizes.options.sort.clone());
    sizes.options.limit=50;
    let size_50=link(&sizes,0,None,sizes.options.sort.clone());
    sizes.options.limit=100;
    let size_100=link(&sizes,0,None,sizes.options.sort.clone());
    let view=PageTemplate {
        title:format!("Data · {}.{}",nav.id.namespace,nav.id.name),
        headers,rows:rendered,
        has_previous:page.has_previous,has_more:page.has_more,
        previous_url,next_url,page_number:page.page.saturating_add(1),
        size_25,size_50,size_100,
        stable:page.stable_order,keyset:page.keyset_enabled,
        truncated_columns:page.truncated_columns,
        limit:nav.options.limit,shown_from,shown_to,
    };
    let body=view.render().map_err(|_|AppError::template())?;
    let mut response=Html(body).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("no-store"));
    Ok(response)
}
