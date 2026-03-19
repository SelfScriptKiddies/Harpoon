/** Pipeline node kind definitions — mirrors Rust types */

export const NODE_KINDS = {
  source: {
    label: 'Source',
    category: 'source',
    color: '#8FE36A',
    description: 'Traffic entry point (TCP/UDP listener)',
    ports: { out: ['default'] },
    fields: [
      { key: 'protocol', type: 'select', options: ['tcp', 'udp'], default: 'tcp' },
      { key: 'listen', type: 'text', placeholder: '0.0.0.0:8080', mono: true },
      { key: 'udp_source_mode', type: 'select', options: ['proxy', 'preserve'], default: 'proxy' },
      { key: 'idle_timeout_secs', type: 'number', default: 30 },
    ],
  },
  tls_terminate: {
    label: 'TLS Terminate',
    category: 'tls',
    color: '#6FAFE8',
    description: 'Accept TLS from client, pass plaintext',
    ports: { in: ['default'], out: ['default'] },
    fields: [
      { key: 'ca_cert', type: 'text', placeholder: '/path/to/ca.pem', mono: true },
      { key: 'ca_key', type: 'text', placeholder: '/path/to/ca-key.pem', mono: true },
    ],
  },
  tls_initiate: {
    label: 'TLS Initiate',
    category: 'tls',
    color: '#6FAFE8',
    description: 'Wrap plaintext in TLS to upstream',
    ports: { in: ['default'], out: ['default'] },
    fields: [
      { key: 'verify_certs', type: 'bool', default: true },
    ],
  },
  filter: {
    label: 'Filter',
    category: 'filter',
    color: '#E6C15A',
    description: 'Match and pass/drop traffic',
    ports: { in: ['default'], out: ['default'] },
    fields: [
      { key: 'kind', type: 'select', options: ['substr', 'bsubstr', 'regex'], default: 'substr' },
      { key: 'pattern', type: 'text', placeholder: 'pattern', mono: true },
      { key: 'direction', type: 'select', options: ['both', 'c2s', 's2c'], default: 'both' },
      { key: 'action', type: 'select', options: ['drop', 'pass', 'tap-only'], default: 'drop' },
    ],
  },
  forward: {
    label: 'Forward',
    category: 'output',
    color: '#70D98A',
    description: 'Proxy to upstream target',
    ports: { in: ['default'] },
    fields: [
      { key: 'target', type: 'text', placeholder: '10.0.0.1:80', mono: true },
      { key: 'tcp_nodelay', type: 'bool', default: true },
    ],
  },
  duplicate: {
    label: 'Duplicate',
    category: 'output',
    color: '#5CCB7B',
    description: 'Send copy to another endpoint',
    ports: { in: ['default'] },
    fields: [
      { key: 'target', type: 'text', placeholder: '10.0.0.2:9090', mono: true },
    ],
  },
  export: {
    label: 'Export',
    category: 'output',
    color: '#9B7BEA',
    description: 'Send events to external sink',
    ports: { in: ['default'] },
    fields: [
      { key: 'kind', type: 'select', options: ['uds', 'tcp'], default: 'tcp' },
      { key: 'path', type: 'text', placeholder: '/tmp/harpoon.sock', mono: true },
      { key: 'addr', type: 'text', placeholder: '127.0.0.1:4000', mono: true },
    ],
  },
  drop: {
    label: 'Drop',
    category: 'output',
    color: '#E26D6D',
    description: 'Discard traffic',
    ports: { in: ['default'] },
    fields: [],
  },
  router: {
    label: 'Router',
    category: 'routing',
    color: '#6FAFE8',
    description: 'Conditional routing by filter match',
    ports: { in: ['default'], out: ['match', 'default'] },
    fields: [
      { key: 'kind', type: 'select', options: ['substr', 'bsubstr', 'regex'], default: 'substr' },
      { key: 'pattern', type: 'text', placeholder: 'pattern', mono: true },
      { key: 'direction', type: 'select', options: ['both', 'c2s', 's2c'], default: 'both' },
    ],
  },
};

export const CATEGORIES = {
  source: { label: 'Sources', color: '#8FE36A' },
  tls: { label: 'TLS', color: '#6FAFE8' },
  filter: { label: 'Filters', color: '#E6C15A' },
  routing: { label: 'Routing', color: '#6FAFE8' },
  output: { label: 'Outputs', color: '#70D98A' },
};

export const PRESETS = [
  {
    id: 'tcp_redirect',
    name: 'TCP Redirect',
    description: 'Simple TCP forward proxy',
    nodes: [
      { id: 1, kind: 'source', config: { protocol: 'tcp', listen: '' } },
      { id: 2, kind: 'forward', config: { target: '' } },
    ],
    edges: [{ from: 1, to: 2 }],
  },
  {
    id: 'udp_relay',
    name: 'UDP Relay',
    description: 'UDP proxy with session table',
    nodes: [
      { id: 1, kind: 'source', config: { protocol: 'udp', listen: '', idle_timeout_secs: 30 } },
      { id: 2, kind: 'forward', config: { target: '' } },
    ],
    edges: [{ from: 1, to: 2 }],
  },
  {
    id: 'tcp_tls_mitm',
    name: 'TCP TLS MITM',
    description: 'Terminate + re-encrypt TLS with plaintext access',
    nodes: [
      { id: 1, kind: 'source', config: { protocol: 'tcp', listen: '' } },
      { id: 2, kind: 'tls_terminate', config: { ca_cert: '', ca_key: '' } },
      { id: 3, kind: 'tls_initiate', config: { verify_certs: true } },
      { id: 4, kind: 'forward', config: { target: '' } },
    ],
    edges: [{ from: 1, to: 2 }, { from: 2, to: 3 }, { from: 3, to: 4 }],
  },
  {
    id: 'tcp_filter_forward',
    name: 'TCP Filter + Forward',
    description: 'TCP proxy with payload filtering',
    nodes: [
      { id: 1, kind: 'source', config: { protocol: 'tcp', listen: '' } },
      { id: 2, kind: 'filter', config: { kind: 'substr', pattern: '', action: 'drop' } },
      { id: 3, kind: 'forward', config: { target: '' } },
    ],
    edges: [{ from: 1, to: 2 }, { from: 2, to: 3 }],
  },
  {
    id: 'tcp_duplicate',
    name: 'TCP Duplicate + Forward',
    description: 'Forward traffic and send a copy to another endpoint',
    nodes: [
      { id: 1, kind: 'source', config: { protocol: 'tcp', listen: '' } },
      { id: 2, kind: 'forward', config: { target: '' } },
      { id: 3, kind: 'duplicate', config: { target: '' } },
    ],
    edges: [{ from: 1, to: 2 }, { from: 1, to: 3 }],
  },
  {
    id: 'tap_only',
    name: 'Tap Only',
    description: 'Export traffic without forwarding',
    nodes: [
      { id: 1, kind: 'source', config: { protocol: 'tcp', listen: '' } },
      { id: 2, kind: 'export', config: { kind: 'tcp', addr: '' } },
    ],
    edges: [{ from: 1, to: 2 }],
  },
];

/** Presets for Simple mode — map directly to form fields */
export const SIMPLE_PRESETS = [
  {
    id: 'tcp_redirect',
    name: 'TCP Redirect',
    icon: '→',
    description: 'Forward TCP traffic to another host/port',
    rule: { protocol: 'tcp', listen: '0.0.0.0:8080', target: '' },
  },
  {
    id: 'udp_relay',
    name: 'UDP Relay',
    icon: '⇄',
    description: 'Relay UDP datagrams with session tracking',
    rule: { protocol: 'udp', listen: '0.0.0.0:5353', target: '8.8.8.8:53', idle_timeout_secs: 30 },
  },
  {
    id: 'tcp_tls_terminate',
    name: 'TLS Terminate',
    icon: '🔓',
    description: 'Terminate TLS, forward plaintext to upstream',
    rule: {
      protocol: 'tcp', listen: '0.0.0.0:8443', target: '',
      tls: { mode: 'terminate', ca_cert: '/etc/harpoon/ca.pem', ca_key: '/etc/harpoon/ca-key.pem' },
    },
  },
  {
    id: 'tcp_tls_mitm',
    name: 'TLS MITM',
    icon: '🔐',
    description: 'Full MITM: terminate client TLS, re-encrypt to upstream',
    rule: {
      protocol: 'tcp', listen: '0.0.0.0:8443', target: '',
      tls: { mode: 'mitm', ca_cert: '/etc/harpoon/ca.pem', ca_key: '/etc/harpoon/ca-key.pem' },
    },
  },
  {
    id: 'tcp_filter',
    name: 'TCP with Filter',
    icon: '⚡',
    description: 'Forward TCP with payload filtering',
    rule: {
      protocol: 'tcp', listen: '0.0.0.0:8080', target: '',
      filters: [{ kind: 'substr', pattern: '', direction: 'c2s', action: 'drop' }],
    },
  },
  {
    id: 'tcp_duplicate',
    name: 'Duplicate Traffic',
    icon: '⑂',
    description: 'Forward and mirror traffic to a second endpoint',
    rule: { protocol: 'tcp', listen: '0.0.0.0:8080', target: '', duplicate: '' },
  },
  {
    id: 'tcp_export',
    name: 'Export to Sink',
    icon: '📤',
    description: 'Forward traffic and export events to external analyzer',
    rule: {
      protocol: 'tcp', listen: '0.0.0.0:8080', target: '',
      exporter: { kind: 'tcp', addr: '127.0.0.1:4000' },
    },
  },
  {
    id: 'blank',
    name: 'Blank Rule',
    icon: '○',
    description: 'Start from scratch',
    rule: { protocol: 'tcp', listen: '', target: '' },
  },
];
