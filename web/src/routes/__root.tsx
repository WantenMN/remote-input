import { createRootRoute, Outlet } from '@tanstack/react-router'
import { useEffect } from 'react'

export const Route = createRootRoute({
  component: RootComponent,
})

function RootComponent() {
  useEffect(() => {
    const vv = window.visualViewport
    if (!vv) return

    function onViewportResize() {
      const viewport = window.visualViewport!
      document.body.style.position = 'fixed'
      document.body.style.width = '100%'
      document.body.style.height = viewport.height + 'px'
      document.body.style.top = viewport.offsetTop + 'px'
    }

    vv.addEventListener('resize', onViewportResize)
    vv.addEventListener('scroll', onViewportResize)
    onViewportResize()

    return () => {
      vv.removeEventListener('resize', onViewportResize)
      vv.removeEventListener('scroll', onViewportResize)
    }
  }, [])

  return <Outlet />
}
