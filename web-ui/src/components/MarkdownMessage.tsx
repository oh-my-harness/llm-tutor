import ReactMarkdown from 'react-markdown'
import remarkBreaks from 'remark-breaks'
import remarkGfm from 'remark-gfm'
import rehypeAutolinkHeadings from 'rehype-autolink-headings'
import rehypeExternalLinks from 'rehype-external-links'
import rehypeKatex from 'rehype-katex'
import rehypeSlug from 'rehype-slug'
import remarkMath from 'remark-math'

interface Props {
  text: string
}

export function MarkdownMessage({ text }: Props) {
  return (
    <div className="markdown-message text-sm">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkBreaks, remarkMath]}
        rehypePlugins={[
          rehypeKatex,
          rehypeSlug,
          [rehypeAutolinkHeadings, { behavior: 'wrap' }],
          [rehypeExternalLinks, { target: '_blank', rel: ['nofollow', 'noopener', 'noreferrer'] }],
        ]}
      >
        {text}
      </ReactMarkdown>
    </div>
  )
}
