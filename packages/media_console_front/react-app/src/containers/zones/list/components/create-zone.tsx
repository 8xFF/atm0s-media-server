import { ZoneDetailCard } from '@/components'
import { Button } from '@/components/ui/button'
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from '@/components/ui/sheet'
import { useConsolesQuery } from '@/hooks'
import { map } from 'lodash'

type Props = {}

export const CreateZone: React.FC<Props> = () => {
  const { data: consoles } = useConsolesQuery({})
  return (
    <Sheet>
      <SheetTrigger asChild>
        <Button size="sm">New Zone</Button>
      </SheetTrigger>
      <SheetContent className="!w-full p-0 sm:w-[540px] md:!w-[600px] md:!max-w-none">
        <SheetHeader className="p-6">
          <SheetTitle>Seed address</SheetTitle>
        </SheetHeader>
        <div className="flex h-[calc(100%-76px)] flex-col gap-4 overflow-y-auto p-6 pt-0">
          {map(consoles?.data, (console) => (
            <ZoneDetailCard item={console} key={console?.node_id} />
          ))}
        </div>
      </SheetContent>
    </Sheet>
  )
}
