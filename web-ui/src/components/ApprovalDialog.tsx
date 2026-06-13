interface ApprovalRequest {
  tool: string
  args: Record<string, unknown>
  requestId: string
}

interface Props {
  request: ApprovalRequest | null
  onDecision: (requestId: string, approved: boolean) => void
}

export function ApprovalDialog({ request, onDecision }: Props) {
  if (!request) return null
  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
      <div className="bg-white rounded-xl shadow-xl p-6 max-w-md w-full mx-4">
        <h2 className="text-lg font-semibold mb-2">Tool Approval Required</h2>
        <p className="text-sm text-gray-600 mb-3">
          The agent wants to execute:{' '}
          <span className="font-mono font-medium">{request.tool}</span>
        </p>
        <pre className="bg-gray-50 rounded p-3 text-xs overflow-auto max-h-40 mb-4">
          {JSON.stringify(request.args, null, 2)}
        </pre>
        <div className="flex gap-3 justify-end">
          <button
            className="px-4 py-2 border rounded text-sm text-gray-700"
            onClick={() => onDecision(request.requestId, false)}
          >
            Deny
          </button>
          <button
            className="px-4 py-2 bg-blue-600 text-white rounded text-sm"
            onClick={() => onDecision(request.requestId, true)}
          >
            Approve
          </button>
        </div>
      </div>
    </div>
  )
}
