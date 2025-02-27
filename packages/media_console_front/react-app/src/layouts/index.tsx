import { Separator } from '@/components/ui/separator'
import { SidebarProvider, SidebarTrigger } from '@/components/ui/sidebar'
import { getCookie } from '@/utils'
import { Outlet } from 'react-router-dom'
import { AppSidebar } from './app-sidebar'
import { Header } from './header'

type Props = {}

export * from './nav-user'

export const Layout: React.FC<Props> = () => {
  const sidebarState = getCookie('sidebar_state')
  const defaultOpen = Boolean(sidebarState) === true
  return (
    <SidebarProvider defaultOpen={defaultOpen}>
      <AppSidebar />
      <main className="bg-background relative flex min-h-svh flex-1 flex-col peer-data-[variant=inset]:min-h-[calc(100svh-(--spacing(4)))] md:peer-data-[variant=inset]:m-2 md:peer-data-[variant=inset]:ml-0 md:peer-data-[variant=inset]:rounded-xl md:peer-data-[variant=inset]:shadow-sm md:peer-data-[variant=inset]:peer-data-[state=collapsed]:ml-2">
        <header className="flex h-16 shrink-0 items-center gap-2">
          <div className="flex items-center gap-2 px-4">
            <SidebarTrigger />
            <Separator orientation="vertical" className="mr-2 h-4" />
            <Header />
          </div>
        </header>
        <div className="flex flex-1 flex-col gap-4 p-4 pt-0">
          <Outlet />
        </div>
      </main>
    </SidebarProvider>
  )
}
