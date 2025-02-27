import { getLocalStorage } from '@/utils'
import { LoaderIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Outlet, useNavigate } from 'react-router-dom'

type Props = {}

export const PrivateProvider: React.FC<Props> = () => {
  const navigate = useNavigate()
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const token = getLocalStorage('token')
    if (!token) {
      setLoading(true)
      navigate('/auth/sign-in', {
        replace: true,
      })
    } else {
      setLoading(false)
    }
  }, [navigate])

  return (
    <>
      {loading ? (
        <div className="flex h-screen w-screen items-center justify-center">
          <LoaderIcon className="animate-spin" />
        </div>
      ) : (
        <Outlet />
      )}
    </>
  )
}
