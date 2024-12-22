import { useToast } from '@/hooks/use-toast'
import { CopyIcon } from 'lucide-react'
import { useMemo } from 'react'
import { useCopyToClipboard } from 'usehooks-ts'

type Props = {
  value: string
}

export const TextCopy: React.FC<Props> = ({ value }) => {
  const { toast } = useToast()
  const [, onCopy] = useCopyToClipboard()
  const filterAddr = useMemo(() => {
    const arr = value.split('/')
    return `${arr[0]}/${arr[1]}/${arr[2]}/${arr[3]}/${arr[4]}/${arr[5]}/.../${arr[arr.length - 2]}/${arr[arr.length - 1]}`
  }, [value])
  return (
    <div className="flex w-fit items-center gap-4 rounded bg-muted p-1">
      <span>{filterAddr}</span>
      <CopyIcon
        size={14}
        className="cursor-pointer"
        onClick={() =>
          onCopy(value).then(() => {
            toast({
              title: 'Copied',
              duration: 2000,
            })
          })
        }
      />
    </div>
  )
}
