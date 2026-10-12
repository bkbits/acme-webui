import { createRouter, createWebHistory } from "vue-router";
import { useAuth } from "../composables/auth";

declare module "vue-router" {
  interface RouteMeta {
    title?: string;
    public?: boolean;
  }
}

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/login",
      name: "login",
      component: () => import("../views/LoginView.vue"),
      meta: { title: "登录", public: true },
    },
    {
      path: "/",
      component: () => import("../layouts/MainLayout.vue"),
      redirect: "/certs",
      children: [
        {
          path: "certs",
          name: "certs",
          component: () => import("../views/CertView.vue"),
          meta: { title: "证书管理" },
        },
        {
          path: "dns/:type",
          name: "dns",
          component: () => import("../views/DnsConfigView.vue"),
          meta: { title: "DNS 运营商配置" },
        },
        // 旧路由兼容
        { path: "providers", redirect: "/dns/ali" },
      ],
    },
    { path: "/:pathMatch(.*)*", redirect: "/" },
  ],
});

router.beforeEach((to) => {
  const auth = useAuth();
  if (!to.meta.public && !auth.token.value) {
    return { path: "/login" };
  }
  if (to.path === "/login" && auth.token.value) {
    return { path: "/" };
  }
  const title = to.meta.title || "";
  document.title = title ? `${title} - ACME WebUI` : "ACME WebUI";
  return true;
});

export default router;
