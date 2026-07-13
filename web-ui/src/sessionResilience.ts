export interface SessionRunItem {
  id: string
  activeRun?: unknown | null
}

export function isCurrentSessionEvent(sourceSessionId: string, currentSessionId: string | null) {
  return sourceSessionId === currentSessionId
}

export function isLatestSessionLoad(
  requestVersion: number,
  currentVersion: number,
  requestedSessionId: string,
  currentSessionId: string | null,
) {
  return requestVersion === currentVersion && requestedSessionId === currentSessionId
}

export function reconcileSessionRunState<T extends SessionRunItem>(
  current: T[],
  incoming: SessionRunItem[],
): T[] {
  const runsBySession = new Map(incoming.map((session) => [session.id, session.activeRun ?? null]))
  return current.map((session) => (
    runsBySession.has(session.id)
      ? { ...session, activeRun: runsBySession.get(session.id) }
      : session
  ))
}
