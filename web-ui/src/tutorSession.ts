export function tutorBindingForCreate(selectedTutorId: string | null | undefined) {
  return { tutor_id: selectedTutorId ?? null }
}
