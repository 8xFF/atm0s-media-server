import { Toaster } from '@/components/ui/toaster'
import { TooltipProvider } from '@/components/ui/tooltip'
import { routes } from '@/routes'
import { Provider } from 'jotai'
import { RouterProvider } from 'react-router-dom'
import { ReactQueryProvider } from './react-query-provider'
import { ThemeProvider } from './theme-provider'

type Props = {}

export const AppProvider: React.FC<Props> = () => {
  return (
    <>
      <ThemeProvider storageKey="vite-ui-theme">
        <ReactQueryProvider>
          <Provider>
            <TooltipProvider>
              <RouterProvider router={routes} />
            </TooltipProvider>
          </Provider>
        </ReactQueryProvider>
      </ThemeProvider>
      <Toaster />
    </>
  )
}
