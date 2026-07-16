interface MarkdownSegment {
  protected: boolean
  text: string
}

/**
 * remark-math recognizes dollar delimiters, while LLMs commonly emit the
 * equivalent LaTeX delimiters. Normalize only Markdown prose so examples in
 * fenced and inline code remain literal.
 */
export function normalizeLatexMathDelimiters(markdown: string) {
  return splitMarkdownCode(markdown)
    .map((segment) => segment.protected ? segment.text : normalizeProseSegment(segment.text))
    .join('')
}

function splitMarkdownCode(markdown: string) {
  const segments: MarkdownSegment[] = []
  let fence: { marker: '`' | '~'; length: number } | null = null
  let inlineCodeLength = 0

  const append = (text: string, protectedSegment: boolean) => {
    if (!text) return
    const last = segments[segments.length - 1]
    if (last?.protected === protectedSegment) {
      last.text += text
    } else {
      segments.push({ protected: protectedSegment, text })
    }
  }

  for (const line of markdown.match(/[^\n]*(?:\n|$)/g) ?? []) {
    if (!line) continue
    const body = line.endsWith('\n') ? line.slice(0, -1) : line
    const ending = line.endsWith('\n') ? '\n' : ''

    if (fence) {
      append(line, true)
      if (isClosingFence(body, fence)) fence = null
      continue
    }

    const openingFence = body.match(/^ {0,3}(`{3,}|~{3,})/)
    if (inlineCodeLength === 0 && openingFence) {
      const token = openingFence[1]!
      fence = { marker: token[0] as '`' | '~', length: token.length }
      append(line, true)
      continue
    }

    let index = 0
    while (index < body.length) {
      if (body[index] !== '`') {
        const nextTick = body.indexOf('`', index)
        const end = nextTick === -1 ? body.length : nextTick
        append(body.slice(index, end), inlineCodeLength > 0)
        index = end
        continue
      }

      let end = index + 1
      while (body[end] === '`') end += 1
      const runLength = end - index
      const protectedSegment = inlineCodeLength > 0 || runLength > 0
      append(body.slice(index, end), protectedSegment)
      if (inlineCodeLength === 0) inlineCodeLength = runLength
      else if (runLength === inlineCodeLength) inlineCodeLength = 0
      index = end
    }
    append(ending, inlineCodeLength > 0)
  }

  return segments
}

function isClosingFence(line: string, fence: { marker: '`' | '~'; length: number }) {
  const escapedMarker = fence.marker === '`' ? '`' : '~'
  const match = line.match(new RegExp(`^ {0,3}(${escapedMarker}{${fence.length},})[ \\t]*\\r?$`))
  return Boolean(match)
}

function normalizeProseSegment(text: string) {
  let output = ''
  let index = 0

  while (index < text.length) {
    const kind = delimiterKindAt(text, index)
    if (!kind) {
      output += text[index]
      index += 1
      continue
    }

    const close = kind === 'inline' ? '\\)' : '\\]'
    const closeIndex = findUnescapedDelimiter(text, close, index + 2)
    if (closeIndex === -1) {
      output += text.slice(index)
      break
    }

    const normalized = kind === 'inline' ? '$' : '$$'
    output += normalized
    output += text.slice(index + 2, closeIndex)
    output += normalized
    index = closeIndex + 2
  }

  return output
}

function delimiterKindAt(text: string, index: number): 'inline' | 'display' | null {
  if (text[index] !== '\\' || isEscapedBackslash(text, index)) return null
  if (text[index + 1] === '(') return 'inline'
  if (text[index + 1] === '[') return 'display'
  return null
}

function findUnescapedDelimiter(text: string, delimiter: '\\)' | '\\]', from: number) {
  let index = text.indexOf(delimiter, from)
  while (index !== -1) {
    if (!isEscapedBackslash(text, index)) return index
    index = text.indexOf(delimiter, index + 2)
  }
  return -1
}

function isEscapedBackslash(text: string, index: number) {
  let preceding = 0
  for (let cursor = index - 1; cursor >= 0 && text[cursor] === '\\'; cursor -= 1) {
    preceding += 1
  }
  return preceding % 2 === 1
}
