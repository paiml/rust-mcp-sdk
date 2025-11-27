# Landing Page Templates for cargo-pmcp

This directory contains landing page templates that can be used with any deployment target (pmcp-run, aws-lambda, cloudflare, etc.).

## Available Templates

### Next.js (`nextjs/`)
- **Framework**: Next.js 14 with App Router
- **Output**: Static export (`output: 'export'`)
- **Styling**: Tailwind CSS
- **Compatible with**: AWS Amplify, Vercel, Netlify, any static hosting

## Template Variables

Templates use `{{VARIABLE_NAME}}` syntax for replacement during initialization:

- `{{SERVER_NAME}}` - MCP server name
- `{{TITLE}}` - Page title
- `{{TAGLINE}}` - Server tagline/subtitle
- `{{DESCRIPTION}}` - Server description
- `{{PRIMARY_COLOR}}` - Brand primary color (hex)
- `{{ENDPOINT}}` - MCP server endpoint URL (optional)

## Adding New Templates

1. Create a new directory under `templates/landing/`
2. Add template files with `{{VARIABLE}}` placeholders
3. Update `cargo-pmcp/src/landing/template.rs` to register the template
4. Document any template-specific requirements

## Testing Templates Locally

When developing cargo-pmcp, templates are cloned from the main repository via sparse checkout to ensure users always get the latest version.
