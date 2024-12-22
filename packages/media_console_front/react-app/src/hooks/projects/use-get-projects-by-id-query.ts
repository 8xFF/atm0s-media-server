import { QueryKey } from '@/apis'
import { useQuery } from '@tanstack/react-query'
import { useParams } from 'react-router-dom'

export const useGetProjectsByIdQuery = () => {
  const params = useParams()
  return useQuery({
    queryKey: [QueryKey.GetProjects, params?.id],
    queryFn: async () => {
      const res = await fetch(`/api/projects/${params?.id}`, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      })
      const data = await res.json()
      return data
    },
    enabled: !!params?.id,
    refetchOnWindowFocus: false,
  })
}
