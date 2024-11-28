import { QueryKey } from '@/apis'
import { useToast } from '@/hooks/use-toast'
import { useMutation, useQueryClient } from '@tanstack/react-query'

type CreateProjectsMutationPayload = {
  data: {
    name: string
  }
}

export const useCreateProjectsMutation = () => {
  const { toast } = useToast()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (payload: CreateProjectsMutationPayload) => {
      const res = await fetch('/api/projects', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(payload?.data),
      })
      const data = await res.json()
      return data
    },
    onSuccess: () => {
      queryClient.refetchQueries({
        queryKey: [QueryKey.GetProjects],
      })
      toast({
        title: 'Project created',
        description: 'Your project has been created successfully.',
        duration: 2000,
      })
    },
    onError: () => {
      toast({
        title: 'Error',
        description: 'An error occurred while creating your project.',
        duration: 2000,
      })
    },
  })
}
