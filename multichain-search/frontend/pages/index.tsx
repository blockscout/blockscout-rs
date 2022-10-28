import { Flex } from '@chakra-ui/react'
import type { GetServerSideProps, InferGetServerSidePropsType, NextPage } from 'next'
import { Network } from '../components/Network'
import { Search } from '../components/Search'
import { Instance } from '../types/proxyResponse'

interface Props {
  instances: Instance[]
}

const Index: NextPage<Props> = ({instances}) => {
  return (
    <>
      <Search initialSearchValue=''/>

      <Flex alignItems="center" flexDirection="column" padding="50" fontSize="24">List of chains:</Flex>
      <Flex alignItems="center" flexDirection="column" gap="5">
        {instances.map((instance) => <>{<Network instance={instance} isBig addUrl></Network>}</>)}
      </Flex>
    </>
    )
}

export default Index



interface InstancesResponse {
  items: Instance[]
}

export const getServerSideProps: GetServerSideProps = async (context) => {
  let url = new URL('/api/v1/instances', process.env.PROXY_HOST).toString();
  let { items }: InstancesResponse = await fetch(url).then((r) => r.json())
  return {
    props: {
      instances: items
    }
  }
}