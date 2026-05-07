import { createRouter, RouterProvider } from '@tanstack/react-router'

import { routeTree } from '@/generated/routeTree.gen'

const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

export function AppRouterProvider() {
  return <RouterProvider router={router} />
}
