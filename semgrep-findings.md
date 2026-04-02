                    
                    
┌──────────────────┐
│ 14 Code Findings │
└──────────────────┘
                                                  
  [36m[22m[24m  crates/cli/src/commands/pack.rs[0m
   ❯❯❱ rust.actix.path-traversal.tainted-path.tainted-path
          [31m[1m[24m❰❰ Blocking ❱❱[0m
          The application builds a file path from potentially untrusted data, which can lead to a path        
          traversal vulnerability. An attacker can manipulate the path which the application uses to access   
          files. If the application does not validate user input and sanitize file paths, sensitive files such
          as configuration or user data can be accessed, potentially creating or overwriting files. To prevent
          this vulnerability, validate and sanitize any input that is used to create references to file paths.
          Also, enforce strict file access controls. For example, choose privileges allowing public-facing    
          applications to access only the required files.                                                     
          Details: https://sg.run/YWX5                                                                        
                                                                                                              
          861┆ std::fs::read_to_string(&pack_yaml_path).context("Failed to read pack.yaml")?;
                                                      
  [36m[22m[24m  crates/cli/src/commands/workflow.rs[0m
   ❯❯❱ rust.actix.path-traversal.tainted-path.tainted-path
          [31m[1m[24m❰❰ Blocking ❱❱[0m
          The application builds a file path from potentially untrusted data, which can lead to a path        
          traversal vulnerability. An attacker can manipulate the path which the application uses to access   
          files. If the application does not validate user input and sanitize file paths, sensitive files such
          as configuration or user data can be accessed, potentially creating or overwriting files. To prevent
          this vulnerability, validate and sanitize any input that is used to create references to file paths.
          Also, enforce strict file access controls. For example, choose privileges allowing public-facing    
          applications to access only the required files.                                                     
          Details: https://sg.run/YWX5                                                                        
                                                                                                              
          188┆ std::fs::read_to_string(action_path).context("Failed to read action YAML file")?;
            ⋮┆----------------------------------------
          223┆ std::fs::read_to_string(&workflow_path).context("Failed to read workflow YAML file")?;
                                         
  [36m[22m[24m  crates/cli/src/wait.rs[0m
   ❯❯❱ javascript.lang.security.detect-insecure-websocket.detect-insecure-websocket
          [31m[1m[24m❰❰ Blocking ❱❱[0m
          Insecure WebSocket Detected. WebSocket Secure (wss) should be used for all WebSocket connections.
          Details: https://sg.run/GWyz                                                                     
                                                                                                           
          483┆ /// - `http://api.example.com:9000` → `ws://api.example.com:8081`
            ⋮┆----------------------------------------
          525┆ Some("ws://api.example.com:8081".to_string())
            ⋮┆----------------------------------------
          529┆ Some("ws://10.0.0.5:8081".to_string())
                                                        
  [36m[22m[24m  crates/common/src/pack_environment.rs[0m
   ❯❯❱ rust.actix.path-traversal.tainted-path.tainted-path
          [31m[1m[24m❰❰ Blocking ❱❱[0m
          The application builds a file path from potentially untrusted data, which can lead to a path        
          traversal vulnerability. An attacker can manipulate the path which the application uses to access   
          files. If the application does not validate user input and sanitize file paths, sensitive files such
          as configuration or user data can be accessed, potentially creating or overwriting files. To prevent
          this vulnerability, validate and sanitize any input that is used to create references to file paths.
          Also, enforce strict file access controls. For example, choose privileges allowing public-facing    
          applications to access only the required files.                                                     
          Details: https://sg.run/YWX5                                                                        
                                                                                                              
          694┆ Path::new(env_path),
            ⋮┆----------------------------------------
          812┆ return Ok(PathBuf::from(validated).exists());
                                                               
  [36m[22m[24m  crates/common/src/pack_registry/installer.rs[0m
   ❯❯❱ rust.actix.ssrf.reqwest-taint.reqwest-taint
          [31m[1m[24m❰❰ Blocking ❱❱[0m
          Untrusted input might be used to build an HTTP request, which can lead to a Server-side request    
          forgery (SSRF) vulnerability. SSRF allows an attacker to send crafted requests from the server side
          to other internal or external systems. SSRF can lead to unauthorized access to sensitive data and, 
          in some cases, allow the attacker to control applications or systems that trust the vulnerable     
          service. To prevent this vulnerability, avoid allowing user input to craft the base request.       
          Instead, treat it as part of the path or query parameter and encode it appropriately. When user    
          input is necessary to prepare the HTTP request, perform strict input validation. Additionally,     
          whenever possible, use allowlists to only interact with expected, trusted domains.                 
          Details: https://sg.run/6D5Y                                                                       
                                                                                                             
          428┆ .get(parsed_url.clone())
                                                 
  [36m[22m[24m  crates/worker/src/artifacts.rs[0m
   ❯❯❱ rust.actix.path-traversal.tainted-path.tainted-path
          [31m[1m[24m❰❰ Blocking ❱❱[0m
          The application builds a file path from potentially untrusted data, which can lead to a path        
          traversal vulnerability. An attacker can manipulate the path which the application uses to access   
          files. If the application does not validate user input and sanitize file paths, sensitive files such
          as configuration or user data can be accessed, potentially creating or overwriting files. To prevent
          this vulnerability, validate and sanitize any input that is used to create references to file paths.
          Also, enforce strict file access controls. For example, choose privileges allowing public-facing    
          applications to access only the required files.                                                     
          Details: https://sg.run/YWX5                                                                        
                                                                                                              
           89┆ let mut file = fs::File::create(&stdout_path)
            ⋮┆----------------------------------------
          123┆ let mut file = fs::File::create(&stderr_path)
            ⋮┆----------------------------------------
          171┆ let mut file = fs::File::create(&result_path)
            ⋮┆----------------------------------------
          217┆ let mut file = fs::File::create(&file_path)
                                               
  [36m[22m[24m  crates/worker/src/service.rs[0m
   ❯❯❱ rust.actix.path-traversal.tainted-path.tainted-path
          [31m[1m[24m❰❰ Blocking ❱❱[0m
          The application builds a file path from potentially untrusted data, which can lead to a path        
          traversal vulnerability. An attacker can manipulate the path which the application uses to access   
          files. If the application does not validate user input and sanitize file paths, sensitive files such
          as configuration or user data can be accessed, potentially creating or overwriting files. To prevent
          this vulnerability, validate and sanitize any input that is used to create references to file paths.
          Also, enforce strict file access controls. For example, choose privileges allowing public-facing    
          applications to access only the required files.                                                     
          Details: https://sg.run/YWX5                                                                        
                                                                                                              
          176┆ config
          177┆     .worker
          178┆     .as_ref()
          179┆     .and_then(|w| w.name.clone())
          180┆     .map(|name| format!("/tmp/attune/artifacts/{}", name))
          181┆     .unwrap_or_else(|| "/tmp/attune/artifacts".to_string()),

