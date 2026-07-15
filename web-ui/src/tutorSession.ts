export function tutorBindingForCreate(selectedTutorId: string | null | undefined) {
  if (selectedTutorId === undefined) {
    throw new Error('请先选择一位导师或临时助手')
  }
  return { tutor_id: selectedTutorId }
}
