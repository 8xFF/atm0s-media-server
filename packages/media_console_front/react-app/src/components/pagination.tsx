import { LIMITS } from '@/utils'
import { map } from 'lodash'
import { ArrowLeftIcon, ArrowRightIcon, ChevronLeftIcon, ChevronRightIcon } from 'lucide-react'
import { Button } from './ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select'

type Props = {
  onFirst: () => void
  onLast: () => void
  onPrev: () => void
  onNext: () => void
  pagination?: {
    total: number
    current: number
  }
  limit: number
  setLimit: (limit: number) => void
}

export const Pagination: React.FC<Props> = ({ onFirst, onLast, onPrev, onNext, pagination, limit, setLimit }) => {
  const current = (pagination?.current || 0) + 1
  return (
    <div className="flex w-full justify-end">
      <div className="flex items-center space-x-6 lg:space-x-8">
        <div className="flex items-center space-x-2">
          <p className="text-sm font-medium">Rows per page</p>
          <Select value={String(limit)} onValueChange={(value) => setLimit(Number(value))}>
            <SelectTrigger className="h-8 w-[70px]">
              <SelectValue placeholder="" />
            </SelectTrigger>
            <SelectContent side="top">
              {map(LIMITS, (pageSize) => (
                <SelectItem key={pageSize} value={`${pageSize}`}>
                  {pageSize}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex w-[100px] items-center justify-center text-sm font-medium">
          Page {current} of {pagination?.total}
        </div>
        <div className="flex items-center space-x-2">
          <Button
            variant="outline"
            className="hidden h-8 w-8 p-0 lg:flex"
            onClick={onFirst}
            disabled={pagination?.current === 0}
          >
            <span className="sr-only">Go to first page</span>
            <ArrowLeftIcon className="h-4 w-4" />
          </Button>
          <Button variant="outline" className="h-8 w-8 p-0" onClick={onPrev} disabled={pagination?.current === 0}>
            <span className="sr-only">Go to previous page</span>
            <ChevronLeftIcon className="h-4 w-4" />
          </Button>
          <Button variant="outline" className="h-8 w-8 p-0" onClick={onNext} disabled={current === pagination?.total}>
            <span className="sr-only">Go to next page</span>
            <ChevronRightIcon className="h-4 w-4" />
          </Button>
          <Button
            variant="outline"
            className="hidden h-8 w-8 p-0 lg:flex"
            onClick={onLast}
            disabled={current === pagination?.total}
          >
            <span className="sr-only">Go to last page</span>
            <ArrowRightIcon className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  )
}
