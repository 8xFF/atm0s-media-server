import cookie from 'js-cookie'

export const setCookie = (name: string, value: never, days = 30) => {
  cookie.set(name, value, { path: '/', expires: days })
}

export const getCookie = (name: string) => {
  return cookie.get(name)
}

export const removeCookie = (name: string) => {
  cookie.remove(name)
}
