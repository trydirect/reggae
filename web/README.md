# reggae website

Marketing website for the [reggae CLI](https://github.com/trydirect/reggae), built with Next.js App Router and exported as a static site for Stacker deployments.

## Local development

```bash
cd web
npm run dev
```

## Production build

```bash
cd web
npm run build
```

## Stacker deployment

Validate the stack configuration and deploy locally first:

```bash
cd web
stacker config validate
stacker deploy --target local
```

The stack is preconfigured for Hetzner cloud defaults in `stacker.yml`:

- provider: `hetzner`
- region: `nbg1`
- size: `cpx11`

When your Stacker login is valid and your cloud key is available, deploy to Hetzner with:

```bash
cd web
stacker login
stacker deploy --target cloud --watch
```

## Routes

- `/` - home page
- `/contact/` - contact page
