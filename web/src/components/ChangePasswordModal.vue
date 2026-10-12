<template>
  <Modal
    :open="open"
    title="修改密码"
    :confirm-loading="loading"
    ok-text="确认修改"
    @ok="onOk"
    @cancel="close"
  >
    <Form ref="formRef" :model="form" :rules="rules" layout="vertical" style="margin-top: 12px">
      <FormItem label="原密码" name="old_password">
        <InputPassword v-model:value="form.old_password" placeholder="请输入原密码" />
      </FormItem>
      <FormItem label="新密码" name="new_password">
        <InputPassword v-model:value="form.new_password" placeholder="至少 6 位" />
      </FormItem>
      <FormItem label="确认新密码" name="confirm">
        <InputPassword v-model:value="form.confirm" placeholder="再次输入新密码" />
      </FormItem>
    </Form>
  </Modal>
</template>

<script setup lang="ts">
import { reactive, ref } from "vue";
import {
  Form,
  FormItem,
  InputPassword,
  Modal,
  message,
  type FormInstance,
  type Rule,
} from "antdv-next";
import { api } from "../api";
import { useAuth } from "../composables/auth";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "update:open", v: boolean): void }>();

const auth = useAuth();
const formRef = ref<FormInstance>();
const loading = ref(false);
const form = reactive({ old_password: "", new_password: "", confirm: "" });

const rules: Record<string, Rule[]> = {
  old_password: [{ required: true, message: "请输入原密码" }],
  new_password: [
    { required: true, message: "请输入新密码" },
    { min: 6, message: "新密码至少 6 位" },
  ],
  confirm: [
    { required: true, message: "请再次输入新密码" },
    {
      validator: (_r: unknown, v: string) =>
        v === form.new_password ? Promise.resolve() : Promise.reject("两次输入不一致"),
    },
  ],
};

const close = () => {
  form.old_password = "";
  form.new_password = "";
  form.confirm = "";
  emit("update:open", false);
};

const onOk = async () => {
  try {
    await formRef.value?.validate();
  } catch {
    return;
  }
  loading.value = true;
  try {
    await api.changePassword(form.old_password, form.new_password);
    message.success("密码已修改，请重新登录");
    close();
    await auth.logout();
    window.location.href = "/login";
  } catch {
    // 错误提示由拦截器统一处理
  } finally {
    loading.value = false;
  }
};
</script>
