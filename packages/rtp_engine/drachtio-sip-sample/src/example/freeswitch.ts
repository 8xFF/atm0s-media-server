import { DRACHTIO_CONFIG, MRF_CONFIG } from 'config'
import * as Srf from 'drachtio-srf'
import { registerParserMiddleware } from 'middlewares'
const TmpSrf = require('drachtio-srf')
const Mrf = require('drachtio-fsmrf')

export function initFreeswitch() {
  const parseUri = TmpSrf.parseUri
  const users = new Map()
  let mediaserver: any

  function createCallee(
    contact: string,
    req: any,
  ): Promise<{ endpoint: any; dialog: Srf.Dialog }> {
    return new Promise(async (resolve, reject) => {
      const ep = await mediaserver.createEndpoint()
      srf.createUAC(
        contact,
        {
          localSdp: ep.local.sdp,
        },
        {
          cbRequest: ((err: any, uacReq: any) => {
            ;(req as any).on('cancel', () => {
              uacReq.cancel((() => {}) as any)
            })
          }) as any,
          cbProvisional: (uacRes) => {
            console.log(`got provisional response: ${uacRes.status}`)
          },
        },
        (err, dialog) => {
          if (err) {
            return reject(err)
          }
          resolve({
            endpoint: ep,
            dialog,
          })
        },
      )
    })
  }

  const srf = new Srf.default()
  srf.connect(DRACHTIO_CONFIG)

  srf.on('connect', (err, hostPort) => {
    if (!err) {
      console.log(`connected to drachtio listening on ${hostPort}`)
    } else {
      console.log(`error connecting to drachtio: `, err)
    }
  })

  const mrf = new Mrf(srf)
  mrf
    .connect(MRF_CONFIG)
    .then((ms: any) => {
      console.log('connected to media server')
      mediaserver = ms
    })
    .catch((err: any) => {
      console.log(`Error connecting to media server: ${err}`)
    })
  ;(srf as any).register(registerParserMiddleware, (req: any, res: any) => {
    const uri = parseUri(req.registration.aor)
    const headers: any = {}
    if (req.registration.type === 'unregister') {
      console.log(`unregistering ${uri.user}`)
      users.delete(uri.user)
    } else {
      const contact = req.registration.contact[0].uri
      users.set(uri.user, contact)
      console.log(`registering ${uri.user}`, contact)
      headers['Contact'] =
        `${req.get('Contact')};expires=${req.registration.expires || 300}`
    }

    res.send(200, {
      headers,
    })
  })

  srf.invite(async (req, res) => {
    const uri = parseUri(req.uri)
    // const from = req.get('From')
    const dest = users.get(uri.user)

    if (!dest) {
      return res.send(486, 'So sorry, busy right now', {})
    }

    res.send(180, 'ringing', {})
    try {
      let uasDialog: Srf.Dialog | undefined = undefined
      let uacDialog: Srf.Dialog | undefined = undefined
      //try to connect to callee
      console.log('create callee endpoint')
      const calleeRes = await createCallee(dest, req)

      const calleeEp = calleeRes.endpoint
      console.log('create uac')
      uacDialog = calleeRes.dialog
      console.log('modify sdp for uac')
      await calleeEp.modify(uacDialog.remote.sdp)

      uacDialog.on('destroy', () => {
        calleeEp.destroy()
        if (uasDialog) uasDialog.destroy()
      })

      //Try to send answer to caller
      console.log('create caller endpoint')
      const callerEp = await mediaserver.createEndpoint({
        remoteSdp: req.body,
      })
      console.log('create uas')
      uasDialog = await srf.createUAS(req, res, {
        localSdp: callerEp.local.sdp,
      })
      uasDialog.on('destroy', () => {
        callerEp.destroy()
        if (uacDialog) uacDialog.destroy()
      })
      console.log('caller ====> callee')
      await callerEp.bridge(calleeEp)
    } catch (err: any) {
      console.log(`Error connecting call: ${err}`)
      if (err.status) {
        console.log('Have error:', err.status, err.reason)
        return res.send(err.status, err.reason, {})
      }
      res.send(500)
    }
  })
}
