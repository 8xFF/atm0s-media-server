import { TNetworkEvent, TNetworkNodeData } from '@/hooks'
import cytoscape from 'cytoscape'
import { throttle } from 'lodash'

const ONLINE_COLOR = '#2ECC40'

type NetworkNode = {
  id: number
  zoneId: number
  info: {
    addr: string
    cpu: number
    memory: number
    disk: number
  }
}

type NetworkConnection = {
  id: number
  source: number
  dest: number
  stats: {
    rttMs: number
  }
}

export class NetworkVisualizationGraph {
  private _cy: cytoscape.Core | undefined
  private _renderer: () => void
  private _nodes: Map<number, NetworkNode> = new Map()
  private _connections: Map<number, NetworkConnection> = new Map()
  private _nodeWillRemove: Set<number> = new Set()
  private _edgesWillRemove: Set<number> = new Set()

  constructor() {
    this._renderer = throttle(this.update, 200)
  }

  init = (element: HTMLDivElement) => {
    if (this._cy) return

    this._cy = cytoscape({
      container: element,
      style: [
        {
          selector: 'node',
          style: {
            'background-color': ONLINE_COLOR,
            label: 'data(label)',
            color: '#fff',
            'text-valign': 'center',
            'text-outline-width': 1,
            'text-outline-color': '#000',
            width: '60px',
            height: '60px',
          },
        },
        {
          selector: 'edge',
          style: {
            width: 2,
            color: '#fff',
            'line-color': '#fff',
            'target-arrow-color': '#fff',
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            'control-point-step-size': 40,
            'text-rotation': 'autorotate',
            'text-margin-y': -5,
            label: 'data(label)',
          },
        },
        {
          selector: '.faded',
          style: {
            opacity: 0.1,
          },
        },
        {
          selector: '.pointed',
          style: {
            'border-color': '#FF6F61',
            'border-width': 2,
            'border-style': 'solid',
          },
        },
      ],
      layout: {
        name: 'circle',
      },
    })

    this._cy.on('click', 'node', (event) => {
      const node = event.target

      this._cy!.elements().removeClass('faded')

      const connectedEdges = node.connectedEdges()
      const connectedNodes = connectedEdges.targets().union(node)
      this._cy!.elements().not(connectedEdges).not(connectedNodes).addClass('faded')

      // this._cy!.fit(connectedNodes, 50)
    })

    this._cy!.on('click', (e) => {
      if (e.target === this._cy!) {
        this._cy!.elements().removeClass('faded')
      }
    })
  }

  onEvent = (event: TNetworkEvent) => {
    switch (event.type) {
      case 'snapshot':
        this.onSnapshot(event.data)
        break
      case 'on_changed':
        this.onNodeChanged(event.data)
        break
      case 'on_removed':
        this.onNodeRemoved(event.data)
        break
    }
    this._renderer()
  }

  private update = () => {
    if (!this._cy) return

    let hasNew = false

    for (const node of this._nodes.values()) {
      const { id } = node
      const nodeEl = this._cy.$id(`${id}`)
      const label = `${node.id} - ${node.info.addr}`
      if (!nodeEl.length) {
        this._cy.add({ data: { id: `${id}`, label } })
        hasNew = true
      }
    }

    for (const conn of this._connections.values()) {
      const edgeId = conn.id
      const edgeEl = this._cy!.getElementById(`${edgeId}`)
      const label = `${conn.stats.rttMs}ms`
      if (!edgeEl.length) {
        hasNew = true
        this._cy.add({
          data: {
            id: `${edgeId}`,
            source: `${conn.source}`,
            target: `${conn.dest}`,
            label: `${label}`,
          },
        })
      } else {
        edgeEl.data({ label })
      }
    }

    const nodesWillRemove = [...this._nodeWillRemove.values()]
    this._nodeWillRemove.clear()

    const edgesWillRemove = [...this._edgesWillRemove.values()]
    this._edgesWillRemove.clear()

    for (const id of edgesWillRemove) {
      this._cy.remove(this._cy.$id(`${id}`))
      this._connections.delete(id)
    }

    for (const id of nodesWillRemove) {
      this._cy.remove(this._cy.$id(`${id}`))
      this._nodes.delete(id)
    }

    if (hasNew) {
      this._cy.fit()
      this._cy.layout({ name: 'circle' }).run()
    }
  }

  private onSnapshot = (data: TNetworkNodeData[]) => {
    for (const node of data) {
      this._nodes.set(node.id, {
        id: node.id,
        zoneId: node.zoneId,
        info: node.info,
      })

      for (const conn of node.connections) {
        this._connections.set(conn.id, {
          id: conn.id,
          source: node.id,
          dest: conn.node,
          stats: {
            rttMs: conn.rttMs,
          },
        })
      }
    }
  }

  private onNodeChanged = (data: TNetworkNodeData) => {
    this._nodes.set(data.id, {
      id: data.id,
      zoneId: data.zoneId,
      info: data.info,
    })

    for (const conn of data.connections) {
      const curr = this._connections.get(conn.id)
      if (curr) {
        this._connections.set(conn.id, {
          ...curr,
          stats: {
            rttMs: conn.rttMs,
          },
        })
      } else {
        this._connections.set(conn.id, {
          id: conn.id,
          source: data.id,
          dest: conn.node,
          stats: {
            rttMs: conn.rttMs,
          },
        })
      }
    }
  }

  private onNodeRemoved = (data: number[]) => {
    for (const id of data) {
      this._nodeWillRemove.add(id)
      for (const conn of this._connections.values()) {
        if (conn.source === id || conn.dest === id) {
          this._edgesWillRemove.add(conn.id)
        }
      }
    }
  }
}
