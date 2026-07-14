export function memoryChangeIds(changes: ReadonlyArray<{ id: string }>): string[] {
  return changes.map((change) => change.id)
}

export function toggleMemoryChange(selectedIds: readonly string[], changeId: string): string[] {
  return selectedIds.includes(changeId)
    ? selectedIds.filter((id) => id !== changeId)
    : [...selectedIds, changeId]
}

export function areAllMemoryChangesSelected(
  changes: ReadonlyArray<{ id: string }>,
  selectedIds: readonly string[],
): boolean {
  if (changes.length === 0) return false
  const selected = new Set(selectedIds)
  return changes.every((change) => selected.has(change.id))
}
