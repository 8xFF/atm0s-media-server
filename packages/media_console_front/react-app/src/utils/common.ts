export const generateRandomString = (length: number) => {
  const date = Date.now().toString()

  let charset = ''
  charset += 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
  charset += 'abcdefghijklmnopqrstuvwxyz'
  charset += date

  let password = ''
  for (let i = 0; i < length; i++) {
    password += charset.charAt(Math.floor(Math.random() * charset.length))
  }
  return password
}
