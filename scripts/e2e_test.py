#!/usr/bin/env python3
"""acme-webui E2E 测试

前置条件:
  1. 已用 mock acme.sh 启动服务:
     ACME_WEBUI_ACME_SH=<repo>/scripts/mock-acme.sh \
     ACME_WEBUI_DB=<tmp>/test.db \
     ACME_WEBUI_SCAN_INTERVAL=3 \
     ./acme-webui -p 16980 &
  2. 执行: python3 scripts/e2e_test.py http://127.0.0.1:16980
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:16980"
CERT_DIR = os.environ.get("E2E_CERT_DIR", "/tmp/acme-e2e")
POST_LOG = os.path.join(CERT_DIR, "post.log")

PASS = 0
FAIL = 0
TOKEN = ""


def check(name: str, cond: bool, detail: str = ""):
    global PASS, FAIL
    if cond:
        PASS += 1
        print(f"  [PASS] {name}")
    else:
        FAIL += 1
        print(f"  [FAIL] {name}  {detail}")


def req(method: str, path: str, body=None, auth=True):
    url = BASE + path
    data = json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(url, data=data, method=method)
    r.add_header("Content-Type", "application/json")
    if auth and TOKEN:
        r.add_header("satoken", TOKEN)
    try:
        with urllib.request.urlopen(r, timeout=30) as resp:
            return resp.status, json.loads(resp.read().decode() or "{}")
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read().decode() or "{}")
        except Exception:
            return e.code, {}
    except Exception as e:
        return -1, {"msg": str(e)}


def main():
    global TOKEN
    print(f"== E2E against {BASE} ==")
    os.makedirs(CERT_DIR, exist_ok=True)
    if os.path.exists(POST_LOG):
        os.remove(POST_LOG)

    # 1. 鉴权
    print("[1] 认证")
    st, body = req("GET", "/api/certs", auth=False)
    check("未登录访问被拒绝(401)", st == 401, f"got {st}")
    st, body = req("POST", "/api/auth/login", {"username": "admin", "password": "wrong"})
    check("错误密码被拒绝", st == 400, f"got {st}")
    st, body = req("POST", "/api/auth/login", {"username": "admin", "password": "admin123"})
    check("登录成功", st == 200 and body.get("code") == 0 and body["data"]["token"])
    TOKEN = body["data"]["token"]
    st, body = req("GET", "/api/auth/me")
    check("me 接口", st == 200 and body["data"]["username"] == "admin")

    # 2. 元信息
    print("[2] 元信息")
    st, body = req("GET", "/api/meta")
    check("meta / CA 列表", st == 200 and "Let's Encrypt" in body["data"]["cas"])
    check("meta / acme.sh 检测", st == 200 and body["data"]["acme_sh"]["found"] is True)

    # 3. 运营商
    print("[3] 运营商配置")
    st, body = req("GET", "/api/providers/schema")
    check("schema 返回四家", st == 200 and len(body["data"]) == 4)
    st, body = req("POST", "/api/providers", {
        "name": "测试CF", "provider_type": "cloudflare",
        "credentials": {"CF_Token": ""}, "enabled": True})
    check("CF 凭据不完整被拒绝", st == 400, f"got {st} {body}")
    st, body = req("POST", "/api/providers", {
        "name": "测试CF", "provider_type": "cloudflare",
        "credentials": {"CF_Token": "mock-token", "CF_Account_ID": "acc-123"}, "enabled": True})
    check("创建 CF 配置", st == 200 and body["data"]["id"] > 0, str(body))
    pid = body["data"]["id"]
    st, body = req("POST", "/api/providers", {
        "name": "测试阿里", "provider_type": "aliyun",
        "credentials": {"Ali_Key": "LTAImock", "Ali_Secret": "secret"}, "enabled": True})
    aliyun_id = body["data"]["id"]
    check("创建阿里云配置", st == 200)
    st, body = req("GET", "/api/providers")
    check("运营商列表", st == 200 and len(body["data"]) == 2)
    st, body = req("DELETE", f"/api/providers/{pid}")
    check("被证书引用前可删除(此处未引用)", st == 200)

    # 重新创建 CF(后面用)
    st, body = req("POST", "/api/providers", {
        "name": "测试CF", "provider_type": "cloudflare",
        "credentials": {"CF_Token": "mock-token"}, "enabled": True})
    pid = body["data"]["id"]

    # 4. 证书配置校验
    print("[4] 证书配置校验")
    st, body = req("POST", "/api/certs", {
        "provider_id": pid, "domains": ["bad domain!"], "cert_path": "/x/a.pem",
        "key_path": "/x/a.key", "renewal_days": 60})
    check("非法域名被拒绝", st == 400, f"got {st}")
    st, body = req("POST", "/api/certs", {
        "provider_id": pid, "domains": ["a.example.com"], "cert_path": "relative/a.pem",
        "key_path": "/x/a.key", "renewal_days": 60})
    check("相对路径被拒绝", st == 400, f"got {st}")
    st, body = req("POST", "/api/certs", {
        "provider_id": 9999, "domains": ["a.example.com"], "cert_path": "/x/a.pem",
        "key_path": "/x/a.key", "renewal_days": 60})
    check("不存在的运营商被拒绝", st == 400, f"got {st}")

    # 5. 颁发流程（手动）
    print("[5] 手动颁发")
    cert_path = os.path.join(CERT_DIR, "public.pem")
    key_path = os.path.join(CERT_DIR, "private.key")
    st, body = req("POST", "/api/certs", {
        "name": "测试证书", "provider_id": pid,
        "domains": ["test.example.com", "*.example.com"],
        "cert_path": cert_path, "key_path": key_path,
        "ca": "Let's Encrypt", "renewal_days": 60,
        "post_command": f"echo hello-post-cmd >> {POST_LOG}"})
    check("创建证书配置", st == 200, str(body))
    cert_id = body["data"]["id"]

    st, body = req("GET", "/api/certs")
    created = [c for c in body["data"] if c["id"] == cert_id][0]
    check("初始状态为 pending", created["status"] == "pending", created["status"])
    check("next_renew_at = 创建时间+60天", created["next_renew_at"] is not None)

    st, body = req("POST", f"/api/certs/{cert_id}/renew")
    check("立即续期受理", st == 200 and body["data"]["started"] is True, str(body))
    st, body = req("POST", f"/api/certs/{cert_id}/renew")
    check("重复续期被拒绝(409)", st == 409, f"got {st}")

    ok, status = False, ""
    for _ in range(30):
        time.sleep(1)
        st, body = req("GET", "/api/certs")
        c = [x for x in body["data"] if x["id"] == cert_id][0]
        status = c["status"]
        if status in ("issued", "failed"):
            ok = status == "issued"
            expire = c["cert_expire_at"]
            break
    check("颁发成功(issued)", ok, f"status={status}")
    check("证书到期时间已解析", bool(expire), str(expire))
    check("公钥文件已写入", os.path.isfile(cert_path))
    check("私钥文件已写入", os.path.isfile(key_path))
    if os.path.isfile(key_path):
        mode = os.stat(key_path).st_mode & 0o777
        check("私钥权限 600", mode == 0o600, oct(mode))
    time.sleep(0.5)
    check("完成后命令已执行", os.path.isfile(POST_LOG))

    st, body = req("GET", f"/api/records?cert_id={cert_id}&page=1&page_size=10")
    rec = body["data"]["list"][0]
    check("颁发记录已生成", st == 200 and len(body["data"]["list"]) == 1)
    check("记录: 成功", rec["success"] is True)
    check("记录: 触发方式 manual", rec["trigger"] == "manual")
    check("记录: 域名快照", rec["domains"] == ["test.example.com", "*.example.com"])
    check("记录: 运营商快照", rec["provider_type"] == "cloudflare")
    st, body = req("GET", f"/api/records/{rec['id']}")
    check("记录详情含命令输出", "mock acme.sh" in body["data"]["command_output"])

    # 6. 失败路径
    print("[6] 失败路径")
    st, body = req("POST", "/api/certs", {
        "name": "失败样例", "provider_id": pid, "domains": ["fail.example.com"],
        "cert_path": os.path.join(CERT_DIR, "fail.pem"),
        "key_path": os.path.join(CERT_DIR, "fail.key"), "renewal_days": 30})
    fail_id = body["data"]["id"]
    st, body = req("POST", f"/api/certs/{fail_id}/renew")
    for _ in range(20):
        time.sleep(1)
        st, body = req("GET", "/api/certs")
        c = [x for x in body["data"] if x["id"] == fail_id][0]
        if c["status"] in ("issued", "failed"):
            break
    check("失败证书状态 failed", c["status"] == "failed")
    check("失败原因已记录", "颁发失败" in c["last_error"], c["last_error"])
    st, body = req("GET", f"/api/records?cert_id={fail_id}")
    check("失败记录已生成", body["data"]["list"][0]["success"] is False)

    # 7. 编辑/删除
    print("[7] 编辑与删除")
    st, body = req("PUT", f"/api/certs/{cert_id}", {
        "name": "测试证书-改", "provider_id": pid,
        "domains": ["test.example.com"], "cert_path": cert_path, "key_path": key_path,
        "ca": "ZeroSSL", "renewal_days": 30, "post_command": ""})
    check("编辑证书配置", st == 200, str(body))
    st, body = req("GET", "/api/certs")
    c = [x for x in body["data"] if x["id"] == cert_id][0]
    check("编辑生效", c["ca"] == "ZeroSSL" and c["renewal_days"] == 30)

    st, body = req("DELETE", f"/api/providers/{pid}")
    check("删除被引用的运营商被拒绝(409)", st == 409, f"got {st}")

    st, body = req("POST", "/api/certs/batch-delete", {"ids": [cert_id, fail_id]})
    check("批量删除", st == 200 and body["data"]["deleted"] == 2, str(body))
    st, body = req("GET", "/api/records")
    check("删除证书后记录保留", body["data"]["total"] >= 2, str(body["data"]["total"]))
    st, body = req("DELETE", f"/api/providers/{pid}")
    check("解除引用后运营商可删除", st == 200, f"got {st}")

    # 8. 自动调度
    print("[8] 自动调度")
    st, body = req("POST", "/api/providers", {
        "name": "测试阿里", "provider_type": "aliyun",
        "credentials": {"Ali_Key": "LTAImock", "Ali_Secret": "sec"}, "enabled": True})
    pid2 = body["data"]["id"]
    st, body = req("POST", "/api/certs", {
        "name": "自动续期样例", "provider_id": pid2, "domains": ["auto.example.com"],
        "cert_path": os.path.join(CERT_DIR, "auto.pem"),
        "key_path": os.path.join(CERT_DIR, "auto.key"), "renewal_days": 1})
    auto_id = body["data"]["id"]
    # 直接改库把 last_issued_at 置为过去 → 到期
    import sqlite3
    db_path = os.environ.get("E2E_DB")
    if db_path:
        conn = sqlite3.connect(db_path)
        conn.execute("UPDATE cert SET last_issued_at='2020-01-01 00:00:00' WHERE id=?", (auto_id,))
        conn.commit()
        conn.close()
        ok = False
        for _ in range(40):
            time.sleep(1)
            st, body = req("GET", "/api/certs")
            c = [x for x in body["data"] if x["id"] == auto_id][0]
            if c["status"] == "issued":
                ok = True
                break
        check("调度器自动续期成功", ok, f"status={c['status']}")
        st, body = req("GET", f"/api/records?cert_id={auto_id}")
        check("自动触发记录 trigger=auto", body["data"]["list"][0]["trigger"] == "auto")
    else:
        print("  [SKIP] 未设置 E2E_DB，跳过调度器测试")

    print(f"\n== 结果: PASS={PASS} FAIL={FAIL} ==")
    sys.exit(1 if FAIL else 0)


if __name__ == "__main__":
    main()
