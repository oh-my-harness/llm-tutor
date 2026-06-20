export type AgentStatusKind = 'idle' | 'thinking' | 'tool' | 'done' | 'error'

export interface AgentStatus {
  kind: AgentStatusKind
  label: string
  detail?: string
}
