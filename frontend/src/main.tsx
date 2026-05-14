import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import { NeonAuthUIProvider } from '@neondatabase/neon-js/auth/react'
import './index.css'
import { authClient } from './stack/client'
import { router } from './router'

// navigate function for NeonAuthUIProvider — uses the router directly
function navigate(href: string) {
  router.navigate(href)
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <NeonAuthUIProvider
      authClient={authClient as any}
      navigate={navigate}
      redirectTo="/"
      social={{ providers: ['google'] }}
      emailOTP
      signUp={{ fields: ['name'] }}
    >
      <RouterProvider router={router} />
    </NeonAuthUIProvider>
  </StrictMode>,
)
