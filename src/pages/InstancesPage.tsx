import { useEffect, useState, useCallback } from 'react';
import {
  Alert as ChakraAlert,
  AlertIcon,
  Box,
  Button,
  Flex,
  HStack,
  Heading,
  Spinner,
  Text,
  VStack,
} from '@chakra-ui/react';
import { RepeatIcon, DeleteIcon } from '@chakra-ui/icons';
import { listInstances, killRunningInstance, type RunningInstance } from '../lib/tauri';

/** L8:运行实例列表页 — 列出 spawn 出去的 MC 进程, 可杀 */
export default function InstancesPage() {
  const [instances, setInstances] = useState<RunningInstance[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [killing, setKilling] = useState<number | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await listInstances();
      setInstances(list);
    } catch (e) {
      setError(`加载失败: ${String(e)}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void reload();
    // 5 秒轮询 — 玩家能看到进程消失 / 新进程出现
    const timer = setInterval(() => void reload(), 5000);
    return () => clearInterval(timer);
  }, [reload]);

  const handleKill = async (pid: number) => {
    setKilling(pid);
    try {
      await killRunningInstance(pid);
      await reload();
    } catch (e) {
      setError(`杀进程失败: ${String(e)}`);
    } finally {
      setKilling(null);
    }
  };

  return (
    <Box maxW="880px" mx="auto">
      <Flex align="baseline" justify="space-between" mb={6}>
        <Box>
          <Heading size="lg" color="gray.800" mb={1}>
            运行中的实例
          </Heading>
          <Text fontSize="sm" color="gray.500">
            启动器 spawn 出去的 Java 进程 · 每 5 秒自动刷新
          </Text>
        </Box>
        <Button
          size="sm"
          variant="ghost"
          leftIcon={<RepeatIcon />}
          onClick={() => void reload()}
          isLoading={loading}
        >
          刷新
        </Button>
      </Flex>

      {error && (
        <ChakraAlert status="error" borderRadius="card" mb={4}>
          <AlertIcon />
          {error}
        </ChakraAlert>
      )}

      {loading && instances.length === 0 ? (
        <Flex justify="center" py={12}>
          <Spinner color="brand.500" />
        </Flex>
      ) : instances.length === 0 ? (
        <ChakraAlert status="info" borderRadius="card" bg="blue.50">
          <AlertIcon />
          <Box>
            <Text fontWeight="700" color="blue.700" fontSize="sm">
              没有运行中的实例
            </Text>
            <Text fontSize="xs" color="blue.600" mt={1}>
              到「下载」页选个版本,点「启动」试试
            </Text>
          </Box>
        </ChakraAlert>
      ) : (
        <VStack spacing={3} align="stretch">
          {instances.map((inst) => (
            <InstanceRow
              key={inst.pid}
              instance={inst}
              onKill={() => void handleKill(inst.pid)}
              killing={killing === inst.pid}
            />
          ))}
        </VStack>
      )}
    </Box>
  );
}

function InstanceRow({
  instance,
  onKill,
  killing,
}: {
  instance: RunningInstance;
  onKill: () => void;
  killing: boolean;
}) {
  const started = new Date(instance.started_at);
  const elapsedMin = Math.floor((Date.now() - started.getTime()) / 60000);
  return (
    <Flex
      align="center"
      gap={4}
      bg="white"
      borderRadius="card"
      border="1px solid"
      borderColor="brand.100"
      boxShadow="card"
      px={4}
      py={3.5}
    >
      <Box flex={1} minW={0}>
        <HStack spacing={2} mb={1}>
          <Text fontWeight="800" fontSize="md" color="gray.800">
            {instance.version_id}
          </Text>
          <Text fontSize="xs" color="gray.400" fontFamily="mono">
            pid {instance.pid}
          </Text>
        </HStack>
        <Text fontSize="xs" color="gray.500" noOfLines={1} fontFamily="mono">
          {instance.java_path}
        </Text>
        <Text fontSize="xs" color="gray.400" mt={1}>
          启动于 {started.toLocaleString('zh-CN')} · 已运行 {elapsedMin} 分钟
        </Text>
      </Box>
      <Button
        size="sm"
        colorScheme="red"
        variant="outline"
        leftIcon={<DeleteIcon />}
        onClick={onKill}
        isLoading={killing}
      >
        杀进程
      </Button>
    </Flex>
  );
}
