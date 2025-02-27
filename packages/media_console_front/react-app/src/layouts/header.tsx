import { useMenu } from '@/hooks'

type Props = {}

export const Header: React.FC<Props> = () => {
  const { isActive } = useMenu()
  return (
    <div>
      <h1 className="flex-1 text-xl font-semibold">{isActive?.title}</h1>
    </div>
  )
}
