#!/usr/bin/env groovy
/**
 * Jenkins Pipeline for IDE1.0
 * 
 * Supports:
 * - Multi-branch pipeline (main, develop, feature/*, release/*)
 * - Multi-platform builds (Linux, Windows, macOS agents)
 * - Parallel execution for speed
 * - Artifact archiving and promotion
 * - Security scanning
 * - Release automation
 * 
 * Prerequisites on Jenkins agents:
 * - Linux: rust, cargo, nodejs, python3, docker, xvfb
 * - Windows: rust, cargo, nodejs, python, chocolatey, wixtoolset
 * - macOS: rust, cargo, nodejs, python, homebrew, create-dmg
 */

@Library('shared-pipeline-lib@main') _

pipeline {
    agent none
    
    options {
        // Timeout for entire pipeline
        timeout(time: 2, unit: 'HOURS')
        // Retry on transient failures
        retry(1)
        // Discard old builds
        buildDiscarder(logRotator(numToKeepStr: '30', artifactNumToKeepStr: '10'))
        // Disable concurrent builds for same branch
        disableConcurrentBuilds()
        // ANSI color support
        ansiColor('xterm')
    }
    
    // ============================================================
    // Environment variables
    // ============================================================
    environment {
        CARGO_TERM_COLOR = 'always'
        RUSTFLAGS = '-D warnings'
        CARGO_INCREMENTAL = '0'
        RUST_BACKTRACE = '1'
        CARGO_HOME = "${WORKSPACE}/.cargo"
        RUSTUP_HOME = "${WORKSPACE}/.rustup"
        PATH = "${env.CARGO_HOME}/bin:${env.PATH}"
        
        // Test isolation
        IDE_SHELL_DESKTOP_TEST_ROOT = "${WORKSPACE}/tmp/ide1_test"
        IDE_SHELL_WEB_TEST_ROOT = "${WORKSPACE}/tmp/ide1_web_test"
        
        // Artifact paths
        ARTIFACT_DIR = "${WORKSPACE}/artifacts"
        DIST_DIR = "${WORKSPACE}/dist"
    }
    
    // ============================================================
    // Parameters
    // ============================================================
    parameters {
        choice(
            name: 'TARGET_PLATFORM',
            choices: ['all', 'linux', 'windows', 'macos'],
            description: 'Target platform for build'
        )
        booleanParam(
            name: 'SKIP_TESTS',
            defaultValue: false,
            description: 'Skip test suite (emergency only)'
        )
        booleanParam(
            name: 'PUBLISH_ARTIFACTS',
            defaultValue: true,
            description: 'Publish artifacts to artifact repository'
        )
        string(
            name: 'RELEASE_VERSION',
            defaultValue: '',
            description: 'Release version (for release builds)'
        )
    }
    
    // ============================================================
    // Stages
    // ============================================================
    stages {
        // ----------------------------------------------------------
        // Stage 1: Checkout & Setup
        // ----------------------------------------------------------
        stage('Checkout & Setup') {
            agent { label 'rust' }
            steps {
                checkout scm: [
                    $class: 'GitSCM',
                    branches: [[name: '*/${BRANCH_NAME}']],
                    doGenerateSubmoduleConfigurations: true,
                    submoduleCfg: [],
                    userRemoteConfigs: [[url: 'https://github.com/UlyssesLeoLee/IDE1.0.git', credentialsId: 'github-token']]
                ]
                
                script {
                    // Create artifact directories
                    sh 'mkdir -p artifacts dist tmp'
                    
                    // Set up Rust toolchain
                    sh '''
                        rustup update stable
                        rustup component add rustfmt clippy
                        rustc --version
                        cargo --version
                    '''
                }
            }
            post {
                always {
                    archiveArtifacts artifacts: 'artifacts/**/*', fingerprint: true, allowEmptyArchive: true
                }
            }
        }
        
        // ----------------------------------------------------------
        // Stage 2: Code Quality (Parallel)
        // ----------------------------------------------------------
        stage('Code Quality') {
            parallel {
                // ------------------------------------------------------
                // Rust fmt + clippy
                // ------------------------------------------------------
                stage('Rust fmt + clippy') {
                    agent { label 'rust' }
                    steps {
                        sh '''
                            cargo fmt --all -- --check
                            cargo clippy --workspace --all-targets --no-deps -- -D warnings
                        '''
                    }
                }
                
                // ------------------------------------------------------
                // Markdown lint
                // ------------------------------------------------------
                stage('Markdown lint') {
                    agent { label 'node' }
                    steps {
                        sh '''
                            npm install -g markdownlint-cli2
                            markdownlint-cli2 "*.md" "docs/**/*.md" \
                                -i "DD-*.md,DD-*_*.md,*需求规格书*.md,*設計書*.md"
                        '''
                    }
                }
                
                // ------------------------------------------------------
                // Security audit (cargo-audit + cargo-deny)
                // ------------------------------------------------------
                stage('Security audit') {
                    agent { label 'rust' }
                    steps {
                        sh '''
                            cargo install cargo-audit cargo-deny --locked
                            cargo audit --deny warnings
                            cargo deny check licenses
                            cargo deny check bans
                            cargo deny check advisories
                        '''
                    }
                }
            }
        }
        
        // ----------------------------------------------------------
        // Stage 3: Build & Test (Parallel per platform)
        // ----------------------------------------------------------
        stage('Build & Test') {
            when {
                not { expression { return params.SKIP_TESTS } }
            }
            parallel {
                // ------------------------------------------------------
                // Linux
                // ------------------------------------------------------
                stage('Linux (ubuntu)') {
                    when { expression { return params.TARGET_PLATFORM in ['all', 'linux'] } }
                    agent { label 'linux && rust' }
                    steps {
                        sh '''
                            sudo apt-get update -qq
                            sudo apt-get install -y -qq \
                                libwebkit2gtk-4.1-dev \
                                libgtk-3-dev \
                                libayatana-appindicator3-dev \
                                librsvg2-dev \
                                libsoup-3.0-dev \
                                libjavascriptcoregtk-4.1-dev \
                                patchelf wget pkg-config libssl-dev cmake \
                                xvfb libxcb1 libxcb-render0 libxcb-shape0 libxcb-xfixes0
                        '''
                        sh '''
                            cargo check --workspace --all-targets
                            cargo test --workspace --all-targets --no-fail-fast
                            cargo build -p ide-shell --bin ide-shell
                            cargo build -p ide-shell-web --bin ide-shell-web
                            cargo build -p ide-shell-desktop --bin ide-shell-desktop
                        '''
                        // Archive Linux binaries
                        archiveArtifacts artifacts: 'target/debug/ide-shell*,target/debug/ide-shell-web*,target/debug/ide-shell-desktop*', fingerprint: true
                    }
                }
                
                // ------------------------------------------------------
                // Windows
                // ------------------------------------------------------
                stage('Windows') {
                    when { expression { return params.TARGET_PLATFORM in ['all', 'windows'] } }
                    agent { label 'windows && rust' }
                    steps {
                        bat '''
                            choco install wixtoolset -y --no-progress
                            refreshenv
                        '''
                        bat '''
                            cargo check --workspace --all-targets
                            cargo test --workspace --all-targets --no-fail-fast
                            cargo build -p ide-shell --bin ide-shell
                            cargo build -p ide-shell-web --bin ide-shell-web
                            cargo build -p ide-shell-desktop --bin ide-shell-desktop
                        '''
                        // Archive Windows binaries
                        archiveArtifacts artifacts: 'target/debug/ide-shell*.exe,target/debug/ide-shell-web*.exe,target/debug/ide-shell-desktop*.exe', fingerprint: true
                    }
                }
                
                // ------------------------------------------------------
                // macOS
                // ------------------------------------------------------
                stage('macOS') {
                    when { expression { return params.TARGET_PLATFORM in ['all', 'macos'] } }
                    agent { label 'macos && rust' }
                    steps {
                        sh '''
                            brew update
                            brew install webkit2gtk gtk+3 ayatana-appindicator librsvg libsoup patchelf wget create-dmg
                        '''
                        sh '''
                            cargo check --workspace --all-targets
                            cargo test --workspace --all-targets --no-fail-fast
                            cargo build -p ide-shell --bin ide-shell
                            cargo build -p ide-shell-web --bin ide-shell-web
                            cargo build -p ide-shell-desktop --bin ide-shell-desktop
                        '''
                        // Archive macOS binaries
                        archiveArtifacts artifacts: 'target/debug/ide-shell*,target/debug/ide-shell-web*,target/debug/ide-shell-desktop*', fingerprint: true
                    }
                }
            }
        }
        
        // ----------------------------------------------------------
        // Stage 4: Integration Tests
        // ----------------------------------------------------------
        stage('Integration Tests') {
            agent { label 'linux && rust' }
            when { not { expression { return params.SKIP_TESTS } } }
            steps {
                sh '''
                    sudo apt-get install -y -qq xvfb libxcb1 libxcb-render0 libxcb-shape0 libxcb-xfixes0
                    cargo test --workspace --tests --no-fail-fast
                    chmod +x tests/st/cli_smoke.sh
                    xvfb-run -a bash tests/st/cli_smoke.sh
                '''
            }
        }
        
        // ----------------------------------------------------------
        // Stage 5: UAT (Playwright e2e)
        // ----------------------------------------------------------
        stage('UAT (Playwright e2e)') {
            agent { label 'linux && node' }
            when { not { expression { return params.SKIP_TESTS } } }
            steps {
                sh '''
                    cargo build -p ide-shell-web --bin ide-shell-web
                '''
                dir('tests/uat') {
                    sh '''
                        npm ci --no-audit --no-fund
                        npx playwright install --with-deps chromium
                        IDE_SHELL_WEB_BIN=../../target/debug/ide-shell-web npx playwright test --reporter=html
                    '''
                }
                archiveArtifacts artifacts: 'tests/uat/playwright-report/**,tests/uat/test-results/**', fingerprint: true
            }
        }
        
        // ----------------------------------------------------------
        // Stage 6: Mock Switch Validation (Python)
        // ----------------------------------------------------------
        stage('Mock Switch Validation') {
            agent { label 'python' }
            steps {
                sh '''
                    pip install -U pip pytest pytest-asyncio pyyaml
                    python3 tests/test_ide1_mock_switch.py -v
                    python3 tests/test_ide1_mock_switch_edge.py -v
                '''
            }
        }
        
        // ----------------------------------------------------------
        // Stage 7: Cross-language Parity (Rust ↔ Python)
        // ----------------------------------------------------------
        stage('Cross-language Parity') {
            agent { label 'linux && rust && python' }
            steps {
                sh '''
                    git clone --depth 1 https://github.com/UlyssesLeoLee/Star.git /tmp/star-checkout
                    cd /tmp/star-checkout
                    git fetch --depth 1 origin cebba99c0b1d1237dd4df0ad9091502f4548689c
                    git checkout cebba99c0b1d1237dd4df0ad9091502f4548689c
                '''
                sh '''
                    echo "=== IDE1.0 x Star Python emitter parity ==="
                    python3 tests/parity/cross_language_parity.py
                '''
            }
        }
        
        // ----------------------------------------------------------
        // Stage 8: Tauri Desktop Builds (Parallel per platform)
        // ----------------------------------------------------------
        stage('Tauri Desktop Builds') {
            when {
                anyOf {
                    expression { return params.TARGET_PLATFORM != 'all' }
                    expression { return currentBuild.currentResult == 'SUCCESS' }
                }
            }
            parallel {
                // ------------------------------------------------------
                // Linux .deb + .AppImage
                // ------------------------------------------------------
                stage('Tauri Linux (.deb + .AppImage)') {
                    when { expression { return params.TARGET_PLATFORM in ['all', 'linux'] } }
                    agent { label 'linux && node && rust' }
                    steps {
                        sh '''
                            sudo apt-get install -y -qq \
                                libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
                                librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
                                patchelf wget rpm dpkg fakeroot libappindicator3-dev
                        '''
                        sh '''
                            cargo check -p ide-shell-desktop --all-targets
                            cargo test -p ide-shell-desktop
                        '''
                        dir('tools') {
                            sh 'npm install --no-audit --no-fund'
                        }
                        dir('crates/ide-shell-desktop') {
                            sh '../../tools/node_modules/.bin/tauri build --bundles deb,appimage --config ./tauri.conf.json'
                        }
                        // Archive artifacts
                        archiveArtifacts artifacts: 'crates/ide-shell-desktop/target/release/bundle/deb/*.deb,crates/ide-shell-desktop/target/release/bundle/appimage/*.AppImage', fingerprint: true
                    }
                }
                
                // ------------------------------------------------------
                // Windows .msi
                // ------------------------------------------------------
                stage('Tauri Windows (.msi)') {
                    when { expression { return params.TARGET_PLATFORM in ['all', 'windows'] } }
                    agent { label 'windows && node && rust' }
                    steps {
                        bat '''
                            choco install wixtoolset -y --no-progress
                            refreshenv
                        '''
                        bat '''
                            cargo check -p ide-shell-desktop --all-targets
                            cargo test -p ide-shell-desktop
                        '''
                        dir('tools') {
                            bat 'npm install --no-audit --no-fund'
                        }
                        dir('crates/ide-shell-desktop') {
                            bat '../../tools/node_modules/.bin/tauri build --bundles msi --config ./tauri.conf.json'
                        }
                        archiveArtifacts artifacts: 'crates/ide-shell-desktop/target/release/bundle/msi/*.msi', fingerprint: true
                    }
                }
                
                // ------------------------------------------------------
                // macOS .dmg
                // ------------------------------------------------------
                stage('Tauri macOS (.dmg)') {
                    when { expression { return params.TARGET_PLATFORM in ['all', 'macos'] } }
                    agent { label 'macos && node && rust' }
                    steps {
                        sh '''
                            brew update
                            brew install webkit2gtk gtk+3 ayatana-appindicator librsvg libsoup patchelf wget create-dmg
                        '''
                        sh '''
                            cargo check -p ide-shell-desktop --all-targets
                            cargo test -p ide-shell-desktop
                        '''
                        dir('tools') {
                            sh 'npm install --no-audit --no-fund'
                        }
                        dir('crates/ide-shell-desktop') {
                            sh '../../tools/node_modules/.bin/tauri build --bundles dmg --config ./tauri.conf.json'
                        }
                        archiveArtifacts artifacts: 'crates/ide-shell-desktop/target/release/bundle/dmg/*.dmg', fingerprint: true
                    }
                }
            }
        }
        
        // ----------------------------------------------------------
        // Stage 9: Performance Benchmarks
        // ----------------------------------------------------------
        stage('Performance Benchmarks') {
            agent { label 'rust' }
            steps {
                sh '''
                    cargo build -p sakura-rs --example bench --release
                    echo "=== sakura-rs core ops throughput ==="
                    cargo run -p sakura-rs --example bench --release 2>&1 | tail -10
                    echo "=== benchmark target: >= 50k ops/s ==="
                    
                    if cargo bench --no-run 2>/dev/null; then
                        cargo bench 2>&1 | tail -20
                    else
                        echo "No criterion benches configured"
                    fi
                '''
            }
        }
        
        // ----------------------------------------------------------
        // Stage 9: Release (only on tag push)
        // ----------------------------------------------------------
        stage('Release') {
            when {
                allOf {
                    branch 'refs/tags/v*'
                    expression { return params.PUBLISH_ARTIFACTS }
                }
            }
            agent { label 'rust' }
            steps {
                script {
                    // Download all platform artifacts
                    sh '''
                        mkdir -p dist
                        # Artifacts would be downloaded from previous stages
                        # or from artifact repository (Nexus/Artifactory/S3)
                    '''
                    
                    // Generate checksums
                    sh '''
                        cd dist
                        sha256sum * > SHA256SUMS.txt
                        cat SHA256SUMS.txt
                    '''
                    
                    // Create GitHub Release
                    sh '''
                        gh release create "${GITHUB_REF_NAME}" \
                            dist/* \
                            --title "IDE1.0 ${GITHUB_REF_NAME}" \
                            --notes-file RELEASE_NOTES.md \
                            --draft=false
                    '''
                }
            }
        }
    }
    
    // ============================================================
    // Post-actions
    // ============================================================
    post {
        always {
            // Clean up workspace
            cleanWs()
            
            // Notify (Slack/Email/DingTalk)
            script {
                if (currentBuild.currentResult == 'FAILURE') {
                    // Send failure notification
                    // slackSend(channel: '#ci-cd', message: "❌ Build failed: ${env.JOB_NAME} #${env.BUILD_NUMBER}")
                } else if (currentBuild.currentResult == 'SUCCESS') {
                    // Send success notification for release builds
                    // if (env.BRANCH_NAME == 'main' || env.BRANCH_NAME.startsWith('refs/tags/')) {
                    //     slackSend(channel: '#releases', message: "✅ Release ready: ${env.JOB_NAME} #${env.BUILD_NUMBER}")
                    // }
                }
            }
        }
        
        success {
            echo '✅ Pipeline completed successfully'
        }
        
        failure {
            echo '❌ Pipeline failed'
        }
        
        unstable {
            echo '⚠️ Pipeline unstable (test failures)'
        }
    }
}

// ============================================================
// Shared Library Functions (in vars/)
// ============================================================
/*
// vars/rustToolchain.groovy
def setupRustToolchain(channel = 'stable') {
    sh """
        rustup update ${channel}
        rustup component add rustfmt clippy
        rustup default ${channel}
    """
}

// vars/buildRust.groovy
def buildRust(targets = ['check', 'test', 'build']) {
    targets.each { target ->
        sh "cargo ${target} --workspace --all-targets"
    }
}

// vars/archiveArtifacts.groovy
def archiveRustArtifacts(pattern = 'target/**/*') {
    archiveArtifacts artifacts: pattern, fingerprint: true, allowEmptyArchive: true
}
*/