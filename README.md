# LangTrans

Qwen2.5-0.5B-Instruct 모델을 사용한 HTTP 번역 서버 (Rust)

코드는 Claude Code로 작성하여 일부분만 수정함. 

구현하는 데에 Claude Pro 구독 상태에서 Opus 4.6으로 진행했을 때 5시간 사이에 세션 사용량을 모두 소진했으며 사용량 초기화 이후 프로젝트 완성 및 버그 수정에 세션 사용량 61%가 추가로 소요함.

## 사전 준비

### 1. 모델 자동 다운로드

**모델은 첫 실행 시 자동으로 HuggingFace에서 다운로드됩니다!** 별도의 수동 다운로드가 필요 없습니다.

**환경변수로 모델 선택 가능:**
```bash
# 기본값: Qwen2.5-0.5B-Instruct (빠른 속도, 적당한 품질)
export LANGTRANS_MODEL_ID="Qwen/Qwen2.5-0.5B-Instruct"

# 더 나은 품질 원하면: 3B 모델 (느리지만 고품질)
export LANGTRANS_MODEL_ID="Qwen/Qwen2.5-3B-Instruct"

# 최고 품질: 7B 모델 (가장 느림, 최고 품질)
export LANGTRANS_MODEL_ID="Qwen/Qwen2.5-7B-Instruct"
```

**성능 (Mac M4 32GB 기준):**
- **0.5B 모델**: 모델 로드 2-5초, 번역 1-3초 ⚡ 권장
- **3B 모델**: 모델 로드 10-20초, 번역 3-7초
- **7B 모델**: 모델 로드 30-60초, 번역 10-20초

**기술 스택:**
- Metal GPU 가속 (Apple Silicon 최적화)
- F32 precision (안정성 우선)
- Safetensors 직접 로딩 (ONNX 변환 불필요)
- Memory-mapped 파일 로딩
- HuggingFace Hub 자동 캐싱 (~/.cache/huggingface/)

### 2. 환경변수 설정

필수:
- `LANGTRANS_ADMIN_ID`: 관리자 계정 ID
- `LANGTRANS_ADMIN_PASSWORD`: 관리자 비밀번호

선택:
- `LANGTRANS_PORT`: 서버 포트 (기본값: 8080)
- `LANGTRANS_BIND_ADDR`: 바인드 주소 (기본값: `0.0.0.0:{PORT}`)
- `LANGTRANS_MODEL_ID`: HuggingFace 모델 ID (기본값: `Qwen/Qwen2.5-0.5B-Instruct`)
- `LANGTRANS_MODEL_PATH`: 모델 캐시 디렉토리 (기본값: `./model`)
- `LANGTRANS_APIKEYS_PATH`: API 키 파일 경로 (기본값: `./api_keys.json`)

## 로컬 실행

```bash
# 빌드
cargo build --release

# 실행
export LANGTRANS_ADMIN_ID=admin
export LANGTRANS_ADMIN_PASSWORD=your_password
cargo run --release
```

## Docker 실행

### Docker Compose (권장)

```bash
# .env 파일 생성
cat > .env << EOF
LANGTRANS_ADMIN_ID=admin
LANGTRANS_ADMIN_PASSWORD=your_secure_password
EOF

# 빌드 및 실행
docker-compose up -d

# 로그 확인
docker-compose logs -f
```

### Docker CLI

```bash
# 빌드
docker build -t langtrans .

# 실행
docker run -d \
  -p 8080:8080 \
  -e LANGTRANS_ADMIN_ID=admin \
  -e LANGTRANS_ADMIN_PASSWORD=your_password \
  -v $(pwd)/model:/app/model:ro \
  -v langtrans-data:/app/data \
  --name langtrans \
  langtrans
```

## API 사용법

### 1. 관리자 페이지에서 API 키 생성

브라우저에서 `http://localhost:8080/admin` 접속 후 로그인하여 API 키 생성

### 2. 번역 API 호출

**GET 요청:**
```bash
curl -H "Authorization: Bearer YOUR_API_KEY" \
  "http://localhost:8080/api/translate?from=en&to=ko&text=Hello"
```

**POST 요청:**
```bash
curl -X POST \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"from":"en","to":"ko","text":"Hello world"}' \
  http://localhost:8080/api/translate
```

### 지원 언어

en (English), es (Spanish), fr (French), de (German), pt (Portuguese), ja (Japanese), ko (Korean), zh (Chinese), ar (Arabic), ru (Russian), hi (Hindi)

## 아키텍처

- **웹 프레임워크**: Axum
- **ML 추론**: Candle (Rust ML framework)
- **모델 포맷**: Safetensors (직접 로딩)
- **하드웨어 가속**: Metal (Apple Silicon), CPU fallback
- **토크나이저**: HuggingFace tokenizers
- **템플릿**: Askama
- **인증**: Bearer token + 쿠키 기반 세션

자세한 내용은 [CLAUDE.md](CLAUDE.md) 참조

## 성능

### Candle + Metal 기반 (현재 버전)
Mac Mini M4 32GB 기준:
- **모델 로드**: 10-20초 (Metal backend 사용)
- **번역 속도**: 1-3초 (`Hello, world!` 영어→한국어)
- **초기 설정**: Git LFS로 모델 다운로드만 필요 (변환 불필요)

### 이전 버전 (ONNX Runtime)
- **모델 로드**: 30초~3분
- **번역 속도**: 3-5초
- **초기 설정**: 5분 (Safetensors → ONNX 변환)

**개선 효과**: 약 2-3배 성능 향상 및 설정 간소화

## 총평
결과물은 `CLAUDE.md`에 기록한 `프로젝트 개요`와 `구성` 문단에 맞춰서 잘 만들어주었지만 YanoljaNEXT-Rosetta-4B 모델의 실행 속도가 기대한 것보다 느린 점이 아쉬웠습니다. 더 좋은 하드웨어에서는 빠르겠지만 그래도 밀리초 단위에서 결과를 내려면 일반적인 하드웨어 정도에서는 어렵지 않을 지?

Opus 4.6이 기본값인 점을 파악하지 못하고 그냥 맡겼더니 세션 사용량이 순식간에 바닥난 점은 아쉽지만 그래도 CLUADE.md 파일에 기록한 대로 구현할 계획을 세워달라는 명령 하나에 원하는 기능을 모두 페이즈를 나눠서 순차적으로 하나하나 다 구현해준 점은 만족스럽습니다. 하지만 Claude Pro를 구독하고 있는 한 다음부턴 Claude Code로 Opus 4.6은 못 쓸 듯.
