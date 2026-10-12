<template>
  <div class="login-page">
    <div class="login-card">
      <div class="login-logo">
        <ShieldCheck :size="26" />
      </div>
      <h1 class="login-title">ACME WebUI</h1>
      <p class="login-sub">acme 证书自动申请续期服务平台</p>

      <Form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <FormItem label="用户名" name="username">
          <Input
            v-model:value="form.username"
            placeholder="请输入用户名"
            size="large"
            @pressEnter="onSubmit"
          >
            <template #prefix><User :size="14" /></template>
          </Input>
        </FormItem>
        <FormItem label="密码" name="password">
          <InputPassword
            v-model:value="form.password"
            placeholder="请输入密码"
            size="large"
            @pressEnter="onSubmit"
          />
        </FormItem>
        <Button type="primary" size="large" block :loading="loading" @click="onSubmit">
          <span class="btn-inner"><LogIn :size="15" /> 登 录</span>
        </Button>
      </Form>

      <p class="login-hint">默认账号 admin / admin123，登录后请及时修改密码</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref } from "vue";
import { useRouter } from "vue-router";
import {
  Button,
  Form,
  FormItem,
  Input,
  InputPassword,
  message,
  type FormInstance,
  type Rule,
} from "antdv-next";
import { LogIn, ShieldCheck, User } from "@lucide/vue";
import { api } from "../api";
import { useAuth } from "../composables/auth";

const router = useRouter();
const auth = useAuth();

const formRef = ref<FormInstance>();
const loading = ref(false);
const form = reactive({ username: "admin", password: "" });
const rules: Record<string, Rule[]> = {
  username: [{ required: true, message: "请输入用户名" }],
  password: [{ required: true, message: "请输入密码" }],
};

const onSubmit = async () => {
  try {
    await formRef.value?.validate();
  } catch {
    return;
  }
  loading.value = true;
  try {
    const res = await api.login(form.username.trim(), form.password);
    auth.set(res.token, res.username);
    message.success("登录成功");
    router.push("/");
  } catch {
    // 错误提示由拦截器统一处理
  } finally {
    loading.value = false;
  }
};
</script>

<style lang="scss" scoped>
.login-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background:
    radial-gradient(ellipse at 20% 20%, rgba(22, 119, 255, 0.18), transparent 45%),
    radial-gradient(ellipse at 80% 85%, rgba(19, 194, 194, 0.14), transparent 45%),
    linear-gradient(135deg, #0f172a 0%, #1e3a5f 55%, #134e4a 100%);
  padding: 24px;
}

.login-card {
  width: 400px;
  max-width: 100%;
  background: var(--ant-color-bg-container, #fff);
  border-radius: 14px;
  padding: 38px 34px 26px;
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.38);
  text-align: center;
}

.login-logo {
  width: 56px;
  height: 56px;
  margin: 0 auto 14px;
  border-radius: 15px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  background: linear-gradient(135deg, #1677ff, #13c2c2);
}

.login-title {
  font-size: 22px;
  margin: 0 0 4px;
  color: var(--ant-color-text, #1f2937);
}

.login-sub {
  color: var(--ant-color-text-tertiary, #8c8c8c);
  font-size: 13px;
  margin: 0 0 26px;
}

.btn-inner {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.login-hint {
  color: #b9bec7;
  font-size: 12px;
  margin: 18px 0 0;
}
</style>
