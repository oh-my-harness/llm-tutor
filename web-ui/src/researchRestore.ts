export interface ResearchReportArtifact {
  type: 'research_report'
  artifact_id?: string
  artifact_store?: string
  title?: string
}

interface RestorableResearchMessage {
  role: 'user' | 'assistant' | 'status'
  text: string
  artifacts?: Array<{
    type: string
    artifact_id?: string
    artifact_store?: string
    title?: string
  }>
  researchTitle?: string
  researchUnavailable?: boolean
}

interface RestorableTraceEntry {
  payload?: Record<string, unknown>
}

export interface ResearchReportTraceData {
  title: string
  markdown: string
}

export function researchReportFromTracePayload(payload: Record<string, unknown>): ResearchReportTraceData | undefined {
  if (payload.kind !== 'tool_result' || payload.tool !== 'create_research_report' || payload.ok === false) return undefined
  const details = payload.details
  if (!details || typeof details !== 'object') return undefined
  const item = details as Record<string, unknown>
  const markdown = typeof item.markdown === 'string' ? item.markdown.trim() : ''
  if (!markdown) return undefined
  const title = typeof item.title === 'string' && item.title.trim()
    ? item.title.trim()
    : titleFromReportMarkdown(markdown)
  return { title, markdown }
}

export function attachRestoredResearchReports<T extends RestorableResearchMessage>(
  messages: T[],
  trace: RestorableTraceEntry[],
): T[] {
  const reportsByRunId = new Map<string, ResearchReportTraceData>()
  const reports: ResearchReportTraceData[] = []
  for (const entry of trace) {
    const payload = entry.payload
    if (!payload) continue
    const report = researchReportFromTracePayload(payload)
    if (!report) continue
    reports.push(report)
    if (typeof payload.run_id === 'string') reportsByRunId.set(payload.run_id, report)
  }

  let fallbackIndex = 0
  return messages.map((message) => {
    if (message.role !== 'assistant') return message
    const artifact = message.artifacts?.find((item) => item.type === 'research_report') as ResearchReportArtifact | undefined
    if (!artifact) return message
    const report = artifact.artifact_id ? reportsByRunId.get(artifact.artifact_id) : reports[fallbackIndex]
    fallbackIndex += 1
    if (!report) {
      return {
        ...message,
        researchTitle: artifact.title,
        researchUnavailable: true,
      }
    }
    return {
      ...message,
      text: report.markdown,
      researchTitle: artifact.title?.trim() || report.title,
      researchUnavailable: false,
    }
  })
}

function titleFromReportMarkdown(markdown: string) {
  const heading = markdown
    .split('\n')
    .map((line) => line.trim())
    .find((line) => /^#{1,6}\s+/.test(line))
  return heading?.replace(/^#{1,6}\s+/, '').trim() || 'Research Report'
}
