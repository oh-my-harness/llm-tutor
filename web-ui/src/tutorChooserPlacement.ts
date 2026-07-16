export interface AnchorRect {
  left: number
  top: number
  bottom: number
  width: number
}

export interface ViewportSize {
  width: number
  height: number
}

export interface TutorChooserPlacement {
  left: number
  width: number
  maxHeight: number
  top?: number
  bottom?: number
}

const viewportMargin = 12
const anchorGap = 8
const preferredMaxHeight = 288
const minimumUsefulHeight = 144
const minimumWidth = 280

export function placeTutorChooser(anchor: AnchorRect, viewport: ViewportSize): TutorChooserPlacement {
  const usableWidth = Math.max(0, viewport.width - viewportMargin * 2)
  const width = Math.min(Math.max(anchor.width, minimumWidth), usableWidth)
  const left = Math.min(
    Math.max(viewportMargin, anchor.left),
    Math.max(viewportMargin, viewport.width - viewportMargin - width),
  )
  const spaceAbove = Math.max(0, anchor.top - anchorGap - viewportMargin)
  const spaceBelow = Math.max(0, viewport.height - anchor.bottom - anchorGap - viewportMargin)
  const openUpward = spaceAbove >= Math.min(preferredMaxHeight, minimumUsefulHeight)
    || spaceAbove > spaceBelow
  const availableHeight = openUpward ? spaceAbove : spaceBelow
  const maxHeight = Math.min(preferredMaxHeight, availableHeight)

  return openUpward
    ? { left, width, maxHeight, bottom: viewport.height - anchor.top + anchorGap }
    : { left, width, maxHeight, top: anchor.bottom + anchorGap }
}
