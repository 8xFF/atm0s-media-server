import { QueryKey } from '@/apis'
import { useToast } from '@/hooks/use-toast'
import { useMutation, useQueryClient } from '@tanstack/react-query'

type DeleteProjectsMutationPayload = {
  id: string
}

export const useDeleteProjectsMutation = () => {
  const { toast } = useToast()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (payload: DeleteProjectsMutationPayload) => {
      const res = await fetch(`/api/projects/${payload?.id}`, {
        method: 'DELETE',
        headers: {
          'Content-Type': 'application/json',
        },
      })
      const data = await res.json()
      return data
    },
    onSuccess: () => {
      queryClient.refetchQueries({
        queryKey: [QueryKey.GetProjects],
      })
      toast({
        title: 'Project deleted',
        description: 'Your project has been deleted successfully.',
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
