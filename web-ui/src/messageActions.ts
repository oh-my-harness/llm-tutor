export type QuoteableMessageRole = 'user' | 'assistant'

export interface ConversationMessageLike {
  role: 'user' | 'assistant' | 'status'
}

export function appendMessageQuote(
  currentInput: string,
  role: QuoteableMessageRole,
  messageText: string,
) {
  const quote = formatMessageQuote(role, messageText)
  if (!quote) return currentInput
  return [currentInput.trimEnd(), quote].filter(Boolean).join('\n\n')
}

export function formatMessageQuote(role: QuoteableMessageRole, messageText: string) {
  const text = messageText.trim()
  if (!text) return ''
  const label = role === 'assistant' ? 'Quoted assistant message' : 'Quoted user message'
  const quotedBody = text.split(/\r?\n/).map((line) => `> ${line}`).join('\n')
  return `> **${label}**\n${quotedBody}`
}

export function previousUserMessageIndex(messages: ConversationMessageLike[], messageIndex: number) {
  for (let index = Math.min(messageIndex - 1, messages.length - 1); index >= 0; index -= 1) {
    if (messages[index]?.role === 'user') return index
  }
  return -1
}
