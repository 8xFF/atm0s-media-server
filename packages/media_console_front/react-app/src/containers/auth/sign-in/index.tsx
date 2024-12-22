import { ImgLogo, ImgSignInBg } from '@/assets'
import { Button } from '@/components/ui/button'
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from '@/components/ui/form'
import { Input } from '@/components/ui/input'
import { useLoginMutation } from '@/hooks'
import { useToast } from '@/hooks/use-toast'
import { setLocalStorage } from '@/utils'
import { zodResolver } from '@hookform/resolvers/zod'
import { useForm } from 'react-hook-form'
import { z } from 'zod'

const formSchema = z.object({
  secret: z.string().min(1, {
    message: 'This field is required',
  }),
})

export const AuthSignIn = () => {
  const { toast } = useToast()
  const { mutate: onLogin, isPending: isPendingLogin } = useLoginMutation()
  const form = useForm<z.infer<typeof formSchema>>({
    resolver: zodResolver(formSchema),
    defaultValues: {
      secret: '',
    },
  })
  const onSubmit = (values: z.infer<typeof formSchema>) => {
    onLogin(
      {
        secret: values.secret,
      },
      {
        onSuccess: async (res) => {
          if (!res.status) {
            return toast({
              title: 'Error',
              description: 'Invalid secret jey',
              duration: 2000,
            })
          }
          setLocalStorage('token', res?.data?.token as string)
          setTimeout(() => {
            window.location.href = '/'
          }, 1000)
        },
      }
    )
  }
  return (
    <div className="flex h-screen w-full items-center justify-center md:flex lg:grid lg:min-h-[600px] lg:grid-cols-2 xl:min-h-[800px]">
      <div className="flex items-center justify-center py-12">
        <div className="mx-auto grid w-[350px] gap-6">
          <div className="flex justify-center">
            <img src={ImgLogo} alt="" className="w-24 rounded border" />
          </div>
          <div className="bg-divide h-[1px] w-full" />
          <div className="grid gap-2 text-center">
            <h1 className="text-3xl font-bold">Login</h1>
            <p className="text-balance text-muted-foreground">Enter your secret key to login to your account</p>
          </div>
          <Form {...form}>
            <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
              <FormField
                control={form.control}
                name="secret"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Secret key</FormLabel>
                    <FormControl>
                      <Input placeholder="Enter your secret key" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <Button loading={isPendingLogin} type="submit" className="w-full">
                Continue
              </Button>
            </form>
          </Form>
        </div>
      </div>
      <div className="hidden bg-muted lg:block">
        <img src={ImgSignInBg} alt="" className="h-screen w-full object-cover" />
      </div>
    </div>
  )
}
