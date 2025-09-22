#[cfg(test)]
mod tests {
    use crate::subprocess::runner::*;
    use crate::subprocess::*;
    use futures::StreamExt;
    use std::time::Duration;

    #[tokio::test]
    async fn test_streaming_stdout() {
        let runner = TokioProcessRunner;
        let command = ProcessCommandBuilder::new("echo")
            .arg("line1")
            .arg("&&")
            .arg("echo")
            .arg("line2")
            .build();

        let result = runner.run_streaming(command).await.unwrap();

        let stdout_lines: Vec<String> = result
            .stdout
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        assert!(stdout_lines.iter().any(|l| l.contains("line1")));
        assert!(stdout_lines.iter().any(|l| l.contains("line2")));

        let status = result.status.await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_streaming_stderr() {
        let runner = TokioProcessRunner;
        let command = ProcessCommandBuilder::new("sh")
            .arg("-c")
            .arg("echo 'error' >&2")
            .build();

        let result = runner.run_streaming(command).await.unwrap();

        let stderr_lines: Vec<String> = result
            .stderr
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        assert!(stderr_lines.iter().any(|l| l.contains("error")));

        let status = result.status.await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_streaming_with_timeout() {
        let runner = TokioProcessRunner;
        let command = ProcessCommandBuilder::new("sleep")
            .arg("10")
            .timeout(Duration::from_millis(100))
            .build();

        let result = runner.run_streaming(command).await.unwrap();
        let status = result.status.await.unwrap();

        assert_eq!(status, ExitStatus::Timeout);
    }

    #[tokio::test]
    async fn test_streaming_with_stdin() {
        let runner = TokioProcessRunner;
        let command = ProcessCommandBuilder::new("cat")
            .stdin("test input\n".to_string())
            .build();

        let result = runner.run_streaming(command).await.unwrap();

        let stdout_lines: Vec<String> = result
            .stdout
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        assert!(stdout_lines.iter().any(|l| l.contains("test input")));

        let status = result.status.await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_mock_streaming() {
        let mut mock = MockProcessRunner::new();

        mock.expect_command("echo")
            .returns_stdout("line1\nline2\nline3")
            .returns_stderr("warning1\nwarning2")
            .returns_success()
            .finish();

        let command = ProcessCommandBuilder::new("echo").build();
        let result = mock.run_streaming(command).await.unwrap();

        let stdout_lines: Vec<String> = result
            .stdout
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(stdout_lines, vec!["line1", "line2", "line3"]);

        let stderr_lines: Vec<String> = result
            .stderr
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(stderr_lines, vec!["warning1", "warning2"]);

        let status = result.status.await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_mock_streaming_with_error() {
        let mut mock = MockProcessRunner::new();

        mock.expect_command("fail")
            .returns_stderr("error message")
            .returns_exit_code(1)
            .finish();

        let command = ProcessCommandBuilder::new("fail").build();
        let result = mock.run_streaming(command).await.unwrap();

        let stderr_lines: Vec<String> = result
            .stderr
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(stderr_lines, vec!["error message"]);

        let status = result.status.await.unwrap();
        assert_eq!(status, ExitStatus::Error(1));
    }

    #[tokio::test]
    async fn test_streaming_line_by_line() {
        let runner = TokioProcessRunner;
        let command = ProcessCommandBuilder::new("sh")
            .arg("-c")
            .arg("for i in 1 2 3; do echo line$i; done")
            .build();

        let mut stream = runner.run_streaming(command).await.unwrap();

        let mut lines = Vec::new();
        while let Some(result) = stream.stdout.next().await {
            if let Ok(line) = result {
                lines.push(line);
            }
        }

        assert_eq!(lines.len(), 3);
        assert!(lines.iter().any(|l| l.contains("line1")));
        assert!(lines.iter().any(|l| l.contains("line2")));
        assert!(lines.iter().any(|l| l.contains("line3")));

        let status = stream.status.await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_streaming_interleaved() {
        let runner = TokioProcessRunner;
        let command = ProcessCommandBuilder::new("sh")
            .arg("-c")
            .arg("echo 'stdout1'; echo 'stderr1' >&2; echo 'stdout2'; echo 'stderr2' >&2")
            .build();

        let result = runner.run_streaming(command).await.unwrap();

        let stdout_fut = result.stdout.collect::<Vec<_>>();
        let stderr_fut = result.stderr.collect::<Vec<_>>();

        let (stdout_results, stderr_results) = tokio::join!(stdout_fut, stderr_fut);

        let stdout_lines: Vec<String> = stdout_results.into_iter().filter_map(|r| r.ok()).collect();

        let stderr_lines: Vec<String> = stderr_results.into_iter().filter_map(|r| r.ok()).collect();

        assert!(stdout_lines.iter().any(|l| l.contains("stdout1")));
        assert!(stdout_lines.iter().any(|l| l.contains("stdout2")));
        assert!(stderr_lines.iter().any(|l| l.contains("stderr1")));
        assert!(stderr_lines.iter().any(|l| l.contains("stderr2")));

        let status = result.status.await.unwrap();
        assert!(status.success());
    }
}
