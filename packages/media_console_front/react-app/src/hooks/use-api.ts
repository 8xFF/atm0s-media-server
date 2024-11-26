import { env } from '@/config/env'
import { getLocalStorage } from '@/utils'
import axios from 'axios'
import { useLogout } from '.'

export const useApi = () => {
  const { onLogout } = useLogout()
  const token = getLocalStorage('token')

  const api = axios.create({
    baseURL: `${env.API_URL}/api`,
    headers: {
      'X-API-Key': token,
    },
  })

  api.interceptors.response.use(
    (response) => {
      return response
    },
    async (error) => {
      if (error.response?.status !== 401) {
        return Promise.reject(error)
      } else {
        onLogout()
      }
    }
  )

  return { api, token }
}
