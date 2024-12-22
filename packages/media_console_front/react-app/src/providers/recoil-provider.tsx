import { RecoilRoot } from 'recoil'
import RecoilNexus from 'recoil-nexus'

type Props = {
  children: React.ReactNode
}

export const RecoilProvider: React.FC<Props> = ({ children }) => {
  return (
    <RecoilRoot>
      <RecoilNexus />
      {children}
    </RecoilRoot>
  )
}
