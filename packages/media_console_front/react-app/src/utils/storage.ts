export const setLocalStorage = (key: string, value: object | string) => {
  if (typeof window === 'undefined') return
  localStorage.setItem(key, JSON.stringify(value))
}

export const getLocalStorage = (key: string) => {
  if (typeof window === 'undefined') return
  const value = localStorage.getItem(key)
  return value ? JSON.parse(value) : null
}

export const removeLocalStorage = (key: string) => {
  if (typeof window === 'undefined') return
  localStorage.removeItem(key)
}
