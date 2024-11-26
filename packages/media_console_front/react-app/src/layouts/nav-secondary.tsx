import { type LucideIcon } from 'lucide-react'
import * as React from 'react'

import { SidebarGroup, SidebarGroupContent, SidebarMenu, SidebarMenuButton, SidebarMenuItem } from '@/components/ui/sidebar'
import { Link } from 'react-router-dom'

type Props = {
  items: {
    title: string
    url: string
    icon: LucideIcon | React.ComponentType<object>
  }[]
} & React.ComponentPropsWithoutRef<typeof SidebarGroup>

export const NavSecondary: React.FC<Props> = ({ items, ...props }) => {
  return (
    <SidebarGroup {...props}>
      <SidebarGroupContent>
        <SidebarMenu>
          {items.map((item) => (
            <SidebarMenuItem key={item.title}>
              <SidebarMenuButton asChild size="sm">
                <Link target="_blank" to={item.url}>
                  <item.icon />
                  <span>{item.title}</span>
                </Link>
              </SidebarMenuButton>
            </SidebarMenuItem>
          ))}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  )
}
