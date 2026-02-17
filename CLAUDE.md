# CLAUDE.md

이 파일은 Claude Code (cluade.ai/code)를 사용하여 이 저장소의 코드를 작업할 때의 가이드입니다.

## 프로젝트 개요

LangTrans 는 Rust 프로젝트 (edition 2024)입니다.

https://huggingface.co/yanolja/YanoljaNEXT-Rosetta-4B 모델을 사용해 일반 텍스트를 번역하여 HTTP로 제공합니다.

해당 모델은 Safetensors 모델이므로 ONNX 모델로 변환하여 사용해야 합니다.

웹 프레임워크는 Axum을 사용합니다.

### 구성
#### 1. API
- 엔드포인트: /api/translate
- HTTP 메서드: GET 및 POST 모두 지원
- 필요 헤더 값: Authorization (Bearer token)
- 인자: from(입력 문자열 언어), to(출력 문자열 언어), text(입력 문자열)
- 반환값: 출력 문자열 (text/plain, YanoljaNEXT-Rosetta-4B 모델로 입력 문자열을 변환한 값)
- 실패하는 경우:
   - 잘못된 API 키 입력: 401 Unauthorized
   - 잘못된 언어 코드 입력: 400 Bad Request

#### 2. 관리자 페이지
- 시작지점: /admin
- 동작: 관리자 페이지를 통해 API 키 관리 (추가, 리보크)
   - API 키를 추가할 때 유효 기간을 지정할 수 있어야 함
   - 로그인 시 5번 연속 로그인에 실패한 IP에 대해서는 로그인을 차단해야 함
      - 이 부분을 DB에 저장할 필요는 없음
  - API 키 목록은 특정 경로에 파일로 저장
- 관리자 계정: LANGTRANS_ADMIN_ID, LANGTRANS_ADMIN_PASSWORD 환경변수에 등록된 계정으로 로그인

## 아키텍처

```
src/
  main.rs           # 진입점, 라우터 조립
  config.rs         # 환경변수 로딩 (Config::from_env)
  state.rs          # AppState (공유 상태)
  error.rs          # AppError enum → Axum IntoResponse
  model/            # 번역 모델 관련
    language.rs     # Language enum (11개 언어 코드)
    prompt.rs       # Rosetta 채팅 템플릿 생성
    inference.rs    # ONNX 추론 엔진 (KV 캐시 자기회귀 생성)
  api/              # 번역 API
    auth.rs         # BearerToken 커스텀 Extractor
    translate.rs    # GET/POST 핸들러
  admin/            # 관리자 페이지
    brute_force.rs  # IP별 로그인 시도 차단
    session.rs      # 쿠키 기반 관리자 세션
    routes.rs       # 관리자 페이지 핸들러
  apikey/
    store.rs        # 파일 기반 API 키 저장소 (JSON)
templates/          # Askama HTML 템플릿
```

핵심 흐름: HTTP 요청 → BearerToken 추출 → API 키 검증 → Language 파싱 → 프롬프트 생성 → ONNX 추론 (spawn_blocking) → text/plain 응답

InferenceEngine은 Mutex<Session>으로 보호되며, translate() 호출 시 Prefill + KV 캐시 자기회귀 루프로 토큰을 생성합니다.

## 환경변수

| 변수 | 기본값 | 필수 |
|------|--------|------|
| `LANGTRANS_ADMIN_ID` | - | O |
| `LANGTRANS_ADMIN_PASSWORD` | - | O |
| `LANGTRANS_PORT` | `8080` | X |
| `LANGTRANS_BIND_ADDR` | `0.0.0.0:{PORT}` | X |
| `LANGTRANS_MODEL_PATH` | `./onnx-model` | X |
| `LANGTRANS_APIKEYS_PATH` | `./api_keys.json` | X |

## 사전 준비: ONNX 모델 변환

```bash
pip install optimum[onnxruntime] transformers torch
optimum-cli export onnx --model yanolja/YanoljaNEXT-Rosetta-4B --task text-generation-with-past ./onnx-model/
```

## 빌드 명령어

- **빌드:** `cargo build`
- **실행:** `cargo run`
- **테스트:** `cargo test`
- **단일 테스트 실행:** `cargo test <test_name>`
- **린트:** `cargo clippy`
- **포매팅:** `cargo fmt`
- **체크 (빠른 컴파일 체크):** `cargo check`
