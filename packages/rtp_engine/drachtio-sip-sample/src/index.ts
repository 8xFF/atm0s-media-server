import { executeFunc } from 'example'
;(async () => {
  try {
    const executor = executeFunc()
    executor()
  } catch (err) {
    console.error(err)
  }
})()
