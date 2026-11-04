import { backendGroups } from './backend';
import { frontendGroups } from './frontend';
import { expansionGroups, sqliteGroups } from './expansion';
import { documents } from './documents';

export { documents };

/**
 * One-time bootstrap order. Later, .docs/ is authoritative: update generated
 * Task records in place, never this seed definition, when priorities change.
 */
const httpGroups = frontendGroups.filter((group) => group.key === 'http');
const uiGroups = frontendGroups.filter((group) => group.key !== 'http');

// Phase order is deliberate: PostgreSQL + SQLite backend and HTTP contracts
// precede UI implementation; MySQL/MariaDB follow the frontend milestone.
export const groups = [
  ...backendGroups,
  ...httpGroups,
  ...sqliteGroups,
  ...uiGroups,
  ...expansionGroups,
];
