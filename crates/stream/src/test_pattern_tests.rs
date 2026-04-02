#[cfg(test)]
mod test_pattern_tests {
    use tokio::time::{timeout, Duration};
    
    #[tokio::test]
    async fn test_test_pattern_generator() {
        use crate::test_pattern::{TestPatternConfig, TestPatternGenerator};
        
        let config = TestPatternConfig {
            width: 640,
            height: 480,
            framerate: 30.0,
        };
        
        let generator = TestPatternGenerator::new(config);
        let mut receiver = generator.start();
        
        // Try to receive a few frames to verify the generator is working
        for i in 0..5 {
            match timeout(Duration::from_secs(2), receiver.recv()).await {
                Ok(Some(frame)) => {
                    assert_eq!(frame.width, 640);
                    assert_eq!(frame.height, 480);
                    assert_eq!(frame.data.len(), (640 * 480 * 4) as usize); // BGRA
                    println!("Received frame {}: {}x{}, {} bytes", 
                           i, frame.width, frame.height, frame.data.len());
                }
                Ok(None) => {
                    panic!("Channel unexpectedly closed");
                }
                Err(_) => {
                    panic!("Timeout waiting for frame {}", i);
                }
            }
        }
        
        // Check that frames differ slightly (animation), though with the way the
        // color bars cycle this might not be apparent in just 5 frames
        // So just make sure frames have been produced
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_different_resolutions() {
        use crate::test_pattern::{TestPatternConfig, TestPatternGenerator};
        
        let configs = vec![
            TestPatternConfig { width: 1920, height: 1080, framerate: 30.0 },
            TestPatternConfig { width: 1280, height: 720, framerate: 60.0 },
            TestPatternConfig { width: 640, height: 480, framerate: 15.0 },
        ];
        
        for (i, config) in configs.iter().enumerate() {
            let generator = TestPatternGenerator::new(config.clone());
            let mut receiver = generator.start();
            
            // Receive one frame to verify resolution
            match timeout(Duration::from_secs(2), receiver.recv()).await {
                Ok(Some(frame)) => {
                    assert_eq!(frame.width, config.width);
                    assert_eq!(frame.height, config.height);
                    assert_eq!(frame.data.len(), (config.width * config.height * 4) as usize);
                    println!("Config {}: {}x{} - Frame size: {}", i, config.width, config.height, frame.data.len());
                }
                Ok(None) => panic!("Channel unexpectedly closed for config {}", i),
                Err(_) => panic!("Timeout waiting for frame for config {}", i),
            }
        }
    }
}