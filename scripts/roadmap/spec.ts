import { backendGroups } from './backend';
import { frontendGroups } from './frontend';
import { expansionGroups } from './expansion';
import { documents } from './documents';

export { documents };

/**
 * One-time bootstrap order. Later, .docs/ is authoritative: update generated
 * Task records in place, never this seed definition, when priorities change.
 */
export const groups = [...backendGroups, ...frontendGroups, ...expansionGroups];
