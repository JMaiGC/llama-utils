# LLAMA API SERVER

> Note: Before reading the following content, please make sure that you are working in an environment of Ubuntu 20.04/22.04 and have installed the following necessary dependencies:
>
> * Rust-stable (>= 1.69.0)
> * Add `wasm32-wasi` target to Rust toolchain by running `rustup target add wasm32-wasi` in the terminal
> * WasmEdge 0.13.4 with GGML Plugin ([Installation](https://github.com/second-state/llama-utils/tree/main#requirements),  [Documentation](https://wasmedge.org/docs/start/install#generic-linux-and-macos))

## Build and run

Now let's build and run the API server.

* Build the `llama-api-server` wasm app:

    ```bash
    git clone https://github.com/second-state/llama-utils.git

    cd api-server

    # build the wasm app
    cargo build -p llama-api-server --target wasm32-wasi --release
    ```

    If the commands are successful, you should find the wasm app in `target/wasm32-wasi/release/llama-api-server.wasm`.

* Download the Llama model of gguf format

  When we run the API server in the next step, we will use the `llama-2-7b-chat.Q5_K_M.gguf` model. You can download it by running the following command:

  ```bash
  curl -LO https://huggingface.co/second-state/Llama-2-7B-Chat-GGUF/resolve/main/llama-2-7b-chat.Q5_K_M.gguf
  ```

* Download the Web UI for chatting

  We provide a front-end Web UI for you to easily interact with the API. You can download and extract it by running:

  ```bash
  curl -LO https://github.com/second-state/chatbot-ui/releases/download/v0.1.0/chatbot-ui.tar.gz
  tar xzf chatbot-ui.tar.gz
  ```

* Run the API server:

  The `-h` or `--help` option can list the available options of the `llama-api-server` wasm app:

  ```console
  ~/llama-utils/api-server$ wasmedge llama-api-server.wasm -h

  Usage: llama-api-server.wasm [OPTIONS]

  Options:
    -s, --socket-addr <IP:PORT>
            Sets the socket address [default: 0.0.0.0:8080]
    -m, --model-name <MODEL-NAME>
            Sets the model name [default: default]
    -a, --model-alias <MODEL-ALIAS>
            Sets the alias name of the model in WasmEdge runtime [default: default]
    -c, --ctx-size <CTX_SIZE>
            Sets the prompt context size [default: 4096]
    -n, --n-predict <N_PRDICT>
            Number of tokens to predict [default: 1024]
    -g, --n-gpu-layers <N_GPU_LAYERS>
            Number of layers to run on the GPU [default: 100]
    -b, --batch-size <BATCH_SIZE>
            Batch size for prompt processing [default: 4096]
    -r, --reverse-prompt <REVERSE_PROMPT>
            Halt generation at PROMPT, return control.
    -p, --prompt-template <TEMPLATE>
            Sets the prompt template. [default: llama-2-chat] [possible values: llama-2-chat, codellama-instruct, mistral-instruct-v0.1, mistrallite, openchat, belle-llama-2-chat, vicuna-chat, chatml, baichuan-2, wizard-coder, zephyr, intel-neural]
        --log-prompts
            Print prompt strings to stdout
        --log-stat
            Print statistics to stdout
        --log-all
            Print all log information to stdout
        --web-ui <WEB_UI>
            Root path for the Web UI files [default: chatbot-ui]
    -h, --help
            Print help
  ```

  Now run the API server with the following command:

  ```bash
  wasmedge --dir .:. --nn-preload default:GGML:AUTO:llama-2-7b-chat.Q5_K_M.gguf llama-api-server.wasm -p llama-2-chat
  ```

  The command above starts the API server on the default socket address. Besides, there are also some other options specified in the command:

  * The `--dir .:.` option specifies the current directory as the root directory of the WASI file system.

  * The `--nn-preload default:GGML:AUTO:llama-2-7b-chat.Q5_K_M.gguf` option specifies the Llama model to be used by the API server. The pattern of the argument is `<name>:<encoding>:<target>:<model path>`. Here, the model used is `llama-2-7b-chat.Q5_K_M.gguf`; and we give it an alias `default` as its name in the runtime environment.

  Please guarantee that the port is not occupied by other processes. If the port specified is available on your machine and the command is successful, you should see the following output in the terminal:

  ```console
  Listening on http://0.0.0.0:8080
  ```

  If the Web UI is ready, you can navigate to `http://127.0.0.1:8080` to open the chatbot, it will interact with the API of your server. 

## Test the API server

`llama-api-server` provides a POST API `/v1/models` to list currently available models. You can use `curl` to test it:

```bash
curl -X POST http://localhost:8080/v1/models -H 'accept:application/json'
```

If the command is successful, you should see the similar output as below in your terminal:

```bash
{
    "object":"list",
    "data":[
        {
            "id":"llama-2-chat",
            "created":1697084821,
            "object":"model",
            "owned_by":"Not specified"
        }
    ]
}
```

Ask a question using OpenAI's JSON message format.

```bash
curl -X POST http://localhost:8080/v1/chat/completions -H 'accept:application/json' -H 'Content-Type: application/json' -d '{"messages":[{"role":"system", "content": "You are a helpful assistant."}, {"role":"user", "content": "Who is Robert Oppenheimer?"}], "model":"llama-2-chat"}'
```

Here is the response.

```bash
{
    "id":"",
    "object":"chat.completion",
    "created":1697092593,
    "model":"llama-2-chat",
    "choices":[
        {
            "index":0,
            "message":{
                "role":"assistant",
                "content":"Robert Oppenheimer was an American theoretical physicist and director of the Manhattan Project, which developed the atomic bomb during World War II. He is widely regarded as one of the most important physicists of the 20th century and is known for his contributions to the development of quantum mechanics and the theory of the atomic nucleus. Oppenheimer was also a prominent figure in the post-war nuclear weapons debate, advocating for international control and regulation of nuclear weapons."
            },
            "finish_reason":"stop"
        }
    ],
    "usage":{
        "prompt_tokens":9,
        "completion_tokens":12,
        "total_tokens":21
    }
}
```

## Models

- [x] [Llama-2-7B-Chat-GGUF](https://huggingface.co/second-state/Llama-2-7B-Chat-GGUF)

- [x] [Llama-2-13B-chat-GGUF](https://huggingface.co/second-state/Llama-2-13B-Chat-GGUF)

- [x] [CodeLlama-13B-GGUF](https://huggingface.co/second-state/CodeLlama-13B-Instruct-GGUF)

- [x] [Mistral-7B-Instruct-v0.1-GGUF](https://huggingface.co/second-state/Mistral-7B-Instruct-v0.1-GGUF)

- [x] [MistralLite-7B-GGUF](https://huggingface.co/second-state/MistralLite-7B-GGUF)

- [x] [OpenChat-3.5-GGUF](https://huggingface.co/second-state/OpenChat-3.5-GGUF)

- [x] [BELLE-Llama2-13B-Chat-0.4M-GGUF](https://huggingface.co/second-state/BELLE-Llama2-13B-Chat-0.4M-GGUF)

- [x] [wizard-vicuna-13B-GGUF](https://huggingface.co/second-state/wizard-vicuna-13B-GGUF)

- [x] [CausalLM-14B](https://huggingface.co/second-state/CausalLM-14B-GGUF)

- [x] [TinyLlama-1.1B-Chat-v0.3](https://huggingface.co/second-state/TinyLlama-1.1B-Chat-v0.3-GGUF)

- [x] [Baichuan2-7B-Chat-GGUF](https://huggingface.co/second-state/Baichuan2-7B-Chat-GGUF)

- [x] [Baichuan2-13B-Chat-GGUF](https://huggingface.co/second-state/Baichuan-13B-Chat-GGUF)

- [x] [OpenHermes-2.5-Mistral-7B-GGUF](https://huggingface.co/second-state/OpenHermes-2.5-Mistral-7B-GGUF)

- [x] [Dolphin-2.2-Yi-34B-GGUF](https://huggingface.co/second-state/Dolphin-2.2-Yi-34B-GGUF)

- [x] [Dolphin-2.2-Mistral-7B-GGUF](https://huggingface.co/second-state/Dolphin-2.2-Mistral-7B-GGUF)

- [x] [Dolphin-2.2.1-Mistral-7B-GGUF](https://huggingface.co/second-state/Dolphin-2.2.1-Mistral-7B)

- [x] [Samantha-1.2-Mistral-7B-GGUF](https://huggingface.co/second-state/Samantha-1.2-Mistral-7B)

- [x] [Dolphin-2.1-Mistral-7B-GGUF](https://huggingface.co/second-state/Dolphin-2.1-Mistral-7B-GGUF)

- [x] [Dolphin-2.0-Mistral-7B-GGUF](https://huggingface.co/second-state/Dolphin-2.0-Mistral-7B-GGUF)

- [x] [WizardLM-1.0-Uncensored-CodeLlama-34B-GGUF](https://huggingface.co/second-state/WizardLM-1.0-Uncensored-CodeLlama-34b)

- [x] [Samantha-1.11-CodeLlama-34B-GGUF](https://huggingface.co/second-state/Samantha-1.11-CodeLlama-34B-GGUF)

- [x] [Samantha-1.11-7B-GGUF](https://huggingface.co/second-state/Samantha-1.11-7B-GGUF)

- [x] [WizardCoder-Python-7B-V1.0-GGUF](https://huggingface.co/second-state/WizardCoder-Python-7B-V1.0)

- [x] [Zephyr-7B-Alpha-GGUF](https://huggingface.co/second-state/Zephyr-7B-Alpha-GGUF)

- [x] [WizardLM-7B-V1.0-Uncensored-GGUF](https://huggingface.co/second-state/WizardLM-7B-V1.0-Uncensored-GGUF)

- [x] [WizardLM-13B-V1.0-Uncensored-GGUF](https://huggingface.co/second-state/WizardLM-13B-V1.0-Uncensored-GGUF)

- [x] [Orca-2-13B-GGUF](https://huggingface.co/second-state/Orca-2-13B-GGUF)

- [x] [Neural-Chat-7B-v3-1-GGUF](https://huggingface.co/second-state/Neural-Chat-7B-v3-1-GGUF)

- [x] [Yi-34B-Chat-GGUF](https://huggingface.co/second-state/Yi-34B-Chat-GGUF)

- [ ] [rpguild-chatml-13B-GGUF](https://huggingface.co/second-state/rpguild-chatml-13B-GGUF)

- [ ] [CodeShell-7B-Chat-GGUF](https://huggingface.co/second-state/CodeShell-7B-Chat-GGUF)
