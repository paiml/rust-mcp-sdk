// Configuration helper for landing page
// Values are baked at build time from environment variables

export interface LandingConfig {
  serverName: string
  endpoint: string
  title: string
  tagline: string
  description: string
}

export function getConfig(): LandingConfig {
  const serverName = process.env.MCP_SERVER_NAME || 'MCP Server'
  const endpoint = process.env.MCP_ENDPOINT || ''

  return {
    serverName,
    endpoint,
    title: `${serverName} - MCP Server`,
    tagline: 'A powerful MCP server for AI assistants',
    description: `Landing page for the ${serverName} MCP server`,
  }
}
