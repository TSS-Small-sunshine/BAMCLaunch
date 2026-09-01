import { useEffect, useState } from 'react';
import {
  Alert as ChakraAlert,
  AlertIcon,
  Box,
  Button,
  Flex,
  HStack,
  Heading,
  Input,
  NumberDecrementStepper,
  NumberIncrementStepper,
  NumberInput,
  NumberInputField,
  NumberInputStepper,
  Text,
  Tooltip,
  VStack,
} from '@chakra-ui/react';
import { RepeatIcon, CheckIcon } from '@chakra-ui/icons';
import { loadSettings, saveSettings, type Settings } from '../lib/tauri';
import { scanJavaInstallations, type JavaCandidate } from '../lib/tauri';

/** L7 设置页 —— Java 路径 / 内存 / 游戏目录 */
export default function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [original, setOriginal] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedToast, setSavedToast] = useState(false);
  const [candidates, setCandidates] = useState<JavaCandidate[]>([]);

  // 初始化:加载 settings
  useEffect(() => {
    void (async () => {
      try {
        const s = await loadSettings();
        setSettings(s);
        setOriginal(s);
      } catch (e) {
        setError(`加载设置失败: ${String(e)}`);
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  if (loading) {
    return (
      <Box maxW="720px" mx="auto">
        <Text color="gray.500">加载中...</Text>
      </Box>
    );
  }

  if (error || !settings) {
    return (
      <Box maxW="720px" mx="auto">
        <ChakraAlert status="error" borderRadius="card">
          <AlertIcon />
          {error ?? '设置未加载'}
        </ChakraAlert>
      </Box>
    );
  }

  const dirty = JSON.stringify(settings) !== JSON.stringify(original);

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      await saveSettings(settings);
      setOriginal(settings);
      setSavedToast(true);
      setTimeout(() => setSavedToast(false), 2000);
    } catch (e) {
      setError(`保存失败: ${String(e)}`);
    } finally {
      setSaving(false);
    }
  };

  const handleReset = () => {
    if (original) setSettings(original);
  };

  const handleClearJava = () => {
    setSettings({
      ...settings,
      java: { path: undefined, version: undefined },
    });
  };

  const handleRescanJava = async () => {
    // 触发 L5 扫描拿候选(用随便一个版本 id;只为扫描,结果不写 version.json)
    try {
      const result = await scanJavaInstallations('__settings_scan__');
      setCandidates(result.candidates);
    } catch (e) {
      setError(`扫描 Java 失败: ${String(e)}`);
    }
  };

  const handlePickCandidate = (c: JavaCandidate) => {
    setSettings({
      ...settings,
      java: { path: c.path, version: c.version },
    });
  };

  const handleRestoreGameDir = () => {
    setSettings({ ...settings, game_dir: undefined });
  };

  return (
    <Box maxW="720px" mx="auto">
      <Heading size="lg" color="gray.800" mb={1}>
        设置
      </Heading>
      <Text fontSize="sm" color="gray.500" mb={6}>
        配置启动器行为 · 改动后点「保存」生效
      </Text>

      {savedToast && (
        <ChakraAlert status="success" borderRadius="card" mb={4}>
          <AlertIcon />
          已保存
        </ChakraAlert>
      )}

      <VStack spacing={6} align="stretch">
        {/* Java 路径 */}
        <Box
          bg="white"
          borderRadius="card"
          border="1px solid"
          borderColor="brand.100"
          boxShadow="card"
          p={5}
        >
          <Heading size="sm" mb={3} color="gray.700">
            Java 路径
          </Heading>
          <Text fontSize="xs" color="gray.500" mb={3}>
            不填则启动时自动选第一个满足版本要求的候选(L5 扫描结果)
          </Text>
          <HStack spacing={2}>
            <Input
              value={settings.java.path ?? ''}
              onChange={(e) =>
                setSettings({
                  ...settings,
                  java: { ...settings.java, path: e.target.value || undefined },
                })
              }
              placeholder="例如 D:\jdk-25\bin\java.exe"
              fontFamily="mono"
              fontSize="sm"
            />
            <Tooltip label="从自动扫描结果选一个" placement="top">
              <Button
                size="sm"
                variant="ghost"
                leftIcon={<RepeatIcon />}
                onClick={() => void handleRescanJava()}
              >
                扫描
              </Button>
            </Tooltip>
            {settings.java.path && (
              <Button size="sm" variant="ghost" onClick={handleClearJava}>
                清空
              </Button>
            )}
          </HStack>
          {settings.java.version != null && (
            <Text fontSize="xs" color="grass.600" mt={2}>
              已记版本号: v{settings.java.version}
            </Text>
          )}
          {candidates.length > 0 && (
            <Box mt={3} p={3} bg="gray.50" borderRadius="md">
              <Text fontSize="xs" fontWeight="700" color="gray.600" mb={2}>
                扫描结果(点选填入)
              </Text>
              <VStack align="stretch" spacing={1}>
                {candidates.slice(0, 8).map((c, i) => (
                  <Flex
                    key={i}
                    align="center"
                    gap={2}
                    fontSize="xs"
                    cursor="pointer"
                    onClick={() => handlePickCandidate(c)}
                    _hover={{ bg: 'gray.100' }}
                    p={1.5}
                    borderRadius="sm"
                  >
                    <Text
                      fontWeight="700"
                      color={c.meets_requirement ? 'grass.600' : 'gray.500'}
                      minW="32px"
                    >
                      v{c.version}
                    </Text>
                    <Text flex={1} fontFamily="mono" color="gray.600" noOfLines={1}>
                      {c.path}
                    </Text>
                  </Flex>
                ))}
              </VStack>
            </Box>
          )}
        </Box>

        {/* JVM 内存 */}
        <Box
          bg="white"
          borderRadius="card"
          border="1px solid"
          borderColor="brand.100"
          boxShadow="card"
          p={5}
        >
          <Heading size="sm" mb={3} color="gray.700">
            JVM 内存
          </Heading>
          <Text fontSize="xs" color="gray.500" mb={3}>
            对应 java 命令参数 <code>-Xms</code> 与 <code>-Xmx</code>(单位 MB)
          </Text>
          <HStack spacing={6}>
            <Box>
              <Text fontSize="xs" color="gray.500" mb={1}>
                初始内存 (Xms)
              </Text>
              <NumberInput
                size="sm"
                min={256}
                max={32 * 1024}
                step={256}
                value={settings.jvm.min_memory_mb}
                onChange={(_, v) =>
                  setSettings({ ...settings, jvm: { ...settings.jvm, min_memory_mb: v } })
                }
                maxW="120px"
              >
                <NumberInputField />
                <NumberInputStepper>
                  <NumberIncrementStepper />
                  <NumberDecrementStepper />
                </NumberInputStepper>
              </NumberInput>
            </Box>
            <Box>
              <Text fontSize="xs" color="gray.500" mb={1}>
                最大内存 (Xmx)
              </Text>
              <NumberInput
                size="sm"
                min={256}
                max={32 * 1024}
                step={256}
                value={settings.jvm.max_memory_mb}
                onChange={(_, v) =>
                  setSettings({ ...settings, jvm: { ...settings.jvm, max_memory_mb: v } })
                }
                maxW="120px"
              >
                <NumberInputField />
                <NumberInputStepper>
                  <NumberIncrementStepper />
                  <NumberDecrementStepper />
                </NumberInputStepper>
              </NumberInput>
            </Box>
          </HStack>
        </Box>

        {/* 游戏目录 */}
        <Box
          bg="white"
          borderRadius="card"
          border="1px solid"
          borderColor="brand.100"
          boxShadow="card"
          p={5}
        >
          <Heading size="sm" mb={3} color="gray.700">
            游戏目录
          </Heading>
          <Text fontSize="xs" color="gray.500" mb={3}>
            不填则使用便携模式(可执行文件旁 .bamcl-dev)。改这里会破坏便携原则,慎用。
          </Text>
          <HStack spacing={2}>
            <Input
              value={settings.game_dir ?? ''}
              onChange={(e) => setSettings({ ...settings, game_dir: e.target.value || undefined })}
              placeholder="留空 = 便携模式(可执行文件旁 .bamcl-dev)"
              fontFamily="mono"
              fontSize="sm"
            />
            {settings.game_dir && (
              <Button size="sm" variant="ghost" onClick={handleRestoreGameDir}>
                恢复默认
              </Button>
            )}
          </HStack>
        </Box>

        {/* 操作按钮 */}
        <Flex justify="flex-end" gap={2} pt={2}>
          <Button size="sm" variant="ghost" onClick={handleReset} isDisabled={!dirty}>
            撤销
          </Button>
          <Button
            size="sm"
            colorScheme="brand"
            leftIcon={<CheckIcon />}
            onClick={() => void handleSave()}
            isLoading={saving}
            isDisabled={!dirty}
          >
            保存
          </Button>
        </Flex>
      </VStack>
    </Box>
  );
}
