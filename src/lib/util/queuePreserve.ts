/**
 * Pure decision logic for preserving the saved queue after a partial restore.
 *
 * The backend's get_tracks_by_ids returns only the ids still present in the
 * local library (src-tauri/src/db/cache.rs), preserving order. Restoring 37 of
 * 40 saved tracks therefore "succeeds", and the debounced autosave would then
 * persist those 37 ids, dropping the other 3 from config.json permanently.
 *
 * These helpers hold no runes and touch no Tauri API so the rule can be unit
 * tested directly.
 */

export interface QueuePreserveState {
  /** True while the on-disk queue is being kept instead of overwritten. */
  active: boolean;
  /** The queue as restored from disk; a subset of the saved queue. */
  restored: string[];
}

export const idlePreserveState: QueuePreserveState = { active: false, restored: [] };

/** True when the restore could not bring back every saved id. */
export function isPartialRestore(requested: number, restored: number): boolean {
  return requested > 0 && restored < requested;
}

/** Begin preserving; `restored` is what the queue looks like right now. */
export function preserveStateFor(restored: readonly string[]): QueuePreserveState {
  return { active: true, restored: [...restored] };
}

/** Same ids in the same order. */
export function sameQueue(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

/** Keep an index inside a queue that may have shrunk, or -1 when empty. */
export function clampQueueIndex(index: number, length: number): number {
  if (length <= 0) return -1;
  if (!Number.isFinite(index)) return -1;
  return Math.min(Math.max(Math.trunc(index), -1), length - 1);
}

export interface QueuePatchDecision {
  state: QueuePreserveState;
  /** True when saved_queue/saved_queue_index must be dropped from the patch. */
  suppress: boolean;
}

/**
 * Decide what to do with one outgoing queue patch.
 *
 * While preserving, the patch is withheld for as long as the incoming queue is
 * still exactly the restored subset (same ids, same order). The first time it
 * differs the user has made a deliberate change - a new queue, a reorder, or a
 * clear - so preservation stops and the new list is written normally.
 */
export function decideQueuePatch(
  state: QueuePreserveState,
  incoming: readonly string[],
): QueuePatchDecision {
  if (!state.active) return { state, suppress: false };
  if (sameQueue(incoming, state.restored)) return { state, suppress: true };
  return { state: idlePreserveState, suppress: false };
}
