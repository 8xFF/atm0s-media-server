import { DefinedInitialDataOptions } from '@tanstack/react-query'

export type TInputQuery<P, D> = {
  payload?: P
  options?: Omit<DefinedInitialDataOptions<D>, 'initialData' | 'queryKey'>
}
