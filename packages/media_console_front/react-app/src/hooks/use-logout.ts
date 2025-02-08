import { removeLocalStorage } from '@/utils'
import { useNavigate } from 'react-router-dom'

export const useLogout = () => {
  const navigate = useNavigate()

  const onLogout = () => {
    removeLocalStorage('token')
    navigate('/auth/sign-in', {
      replace: true,
    })
  }

  return { onLogout }
}
