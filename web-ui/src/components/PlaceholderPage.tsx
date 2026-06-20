import type { ReactNode } from 'react'

interface Props {
  title: string
  description: string
  children?: ReactNode
}

export function PlaceholderPage({ title, description, children }: Props) {
  return (
    <main className="flex-1 overflow-y-auto bg-gray-50">
      <div className="mx-auto max-w-4xl px-6 py-6">
        <div className="border border-gray-200 bg-white p-6">
          <h2 className="text-xl font-semibold text-gray-900">{title}</h2>
          <p className="mt-2 text-sm leading-6 text-gray-600">{description}</p>
          {children && <div className="mt-6">{children}</div>}
        </div>
      </div>
    </main>
  )
}
